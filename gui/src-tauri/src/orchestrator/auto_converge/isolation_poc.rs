//! Bounded Restricted-Token Filesystem Proof-of-Concept Harness.
//!
//! Corrective implementation addressing §149.2 and §151.2 requirements for Candidate D:
//! 1. Controller process query and `OpenProcessToken` (`TOKEN_DUPLICATE`) are independent outcomes;
//!    prerequisite failure maps to `NotAttempted`; only raw OS error 5 from `OpenProcessToken` qualifies as denial.
//! 2. Denial classification requires exact raw OS error 5 (`ERROR_ACCESS_DENIED`); generic
//!    `PermissionDenied` without raw OS code is classified as `FailedOther(-1)`.
//! 3. Documented least-privilege desktop rights conforming to Microsoft `CreateDesktopExW` API contract:
//!    child rights (`0x00C3`), controller rights (`0x000600C3`), and SYSTEM rights (`0x00020041`).
//! 4. Centralized post-mutation failure handling: all errors attempt defined restoration once, then preserve
//!    the exact owned `TempDir` fixture whenever uncertainty or failure is present.
//! 5. Token evidence queried structurally (`TokenUser`, `TokenRestrictedSids`, `TokenPrivileges`,
//!    `TokenIntegrityLevel`, `IsTokenRestricted`) with aligned storage; explicit validation gates PASS.
//! 6. Canary baseline records exact bytes/hash and metadata (length, file kind, attributes);
//!    metadata changes fail closed through a pure comparator.
//! 7. Ordered-component path inspection evaluator enforcing contiguous absent suffix and failing closed.
//! 8. Environment block keys restricted to ASCII, validated, and sorted case-insensitively with double-NUL.
//! 9. Pure offline unit tests covering all pure decision functions and seams.
//! 10. Native tests remain `#[ignore]`; two Stage 2 attempts reached no filesystem-boundary evidence.
//! 11. The Stage 2 harness temporarily grants the unique restricting SID minimal Window Station
//!     rights and restores/verifies the captured DACL before fixture recovery or cleanup.

#![cfg(all(windows, test))]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, SetHandleInformation, ERROR_ACCESS_DENIED,
    ERROR_BROKEN_PIPE, ERROR_SUCCESS, FALSE, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, GetSecurityInfo, SetEntriesInAclW, SetNamedSecurityInfoW,
    SetSecurityInfo, EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, REVOKE_ACCESS,
    SE_FILE_OBJECT, SE_WINDOW_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN,
};
use windows_sys::Win32::Security::{
    AclSizeInformation, AllocateAndInitializeSid, CreateRestrictedToken, EqualSid, FreeSid, GetAce,
    GetAclInformation, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    GetSecurityDescriptorLength, GetTokenInformation, IsTokenRestricted, TokenIntegrityLevel,
    TokenPrivileges, TokenRestrictedSids, TokenUser, ACCESS_ALLOWED_ACE, ACCESS_DENIED_ACE,
    ACE_HEADER, ACL, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, DISABLE_MAX_PRIVILEGE,
    PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, SID_AND_ATTRIBUTES, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DUPLICATE, TOKEN_GROUPS, TOKEN_MANDATORY_LABEL, TOKEN_PRIVILEGES, TOKEN_QUERY,
    TOKEN_USER, WRITE_RESTRICTED,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::StationsAndDesktops::{
    CloseDesktop, CloseWindowStation, CreateDesktopW, GetProcessWindowStation,
    GetUserObjectInformationW, OpenWindowStationW, HDESK, UOI_NAME,
};
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
use windows_sys::Win32::System::Threading::{
    CreateProcessAsUserW, DeleteProcThreadAttributeList, GetCurrentProcess, GetCurrentProcessId,
    GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcess, OpenProcessToken,
    ResumeThread, TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, PROCESS_DUP_HANDLE,
    PROCESS_INFORMATION, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE, PROCESS_VM_WRITE, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESHOWWINDOW,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW,
};

pub(crate) type PocResult<T> = Result<T, String>;

pub(crate) const POC_CHILD_TEST_FULL_NAME: &str =
    "orchestrator::auto_converge::isolation_poc::test_poc_child_entrypoint";

// Documented least-privilege desktop access masks conforming to Microsoft Win32 API contracts:
//
// Desktop-specific rights:
//   DESKTOP_READOBJECTS  = 0x0001
//   DESKTOP_CREATEWINDOW = 0x0002
//   DESKTOP_ENUMERATE    = 0x0040
//   DESKTOP_WRITEOBJECTS = 0x0080
// Standard rights:
//   READ_CONTROL         = 0x00020000
//   WRITE_DAC            = 0x00040000
//
// 1. Child rights: Sandboxed child requires rights to read desktop objects, create windows/console,
//    enumerate the desktop, and write desktop objects:
//    0x0001 | 0x0002 | 0x0040 | 0x0080 = 0x00C3.
pub(crate) const DESKTOP_CHILD_RIGHTS: u32 = 0x00C3;

// 2. Controller rights: Controller requires child rights plus standard rights READ_CONTROL and WRITE_DAC
//    to inspect and configure the desktop security descriptor.
//    Per Microsoft CreateDesktopExW / CreateDesktop documentation:
//    "Note that if you request READ_CONTROL, WRITE_DAC, or WRITE_OWNER, you must also specify
//     the DESKTOP_READOBJECTS and DESKTOP_WRITEOBJECTS access rights."
//    Combining DESKTOP_CHILD_RIGHTS (0x00C3) with READ_CONTROL (0x00020000) and WRITE_DAC (0x00040000)
//    yields 0x000600C3, fully satisfying the CreateDesktopW / CreateDesktopExW contract.
pub(crate) const DESKTOP_CONTROLLER_RIGHTS: u32 = 0x000600C3;

// 3. SYSTEM rights: Minimal enumeration, read objects, and read control rights:
//    0x0001 | 0x0040 | 0x00020000 = 0x00020041.
pub(crate) const DESKTOP_SYSTEM_RIGHTS: u32 = 0x00020041;

/// Evaluates opt-in environment variable value purely.
/// Refuses execution unless exactly "1".
pub(crate) fn evaluate_poc_opt_in(val: Option<&str>) -> bool {
    matches!(val, Some("1"))
}

/// Builds the child command line using the exact fully qualified libtest name.
pub(crate) fn build_child_test_command(exe_path: &Path) -> String {
    format!(
        "\"{}\" {} --exact --ignored --nocapture",
        exe_path.display(),
        POC_CHILD_TEST_FULL_NAME
    )
}

/// Builds a noninteractive CDB command line that stops at first-chance loader
/// initialization failures and records the exception context and owning module.
/// Output is captured through the bounded redirected standard-output pipe.
pub(crate) fn build_cdb_child_command(
    cdb_path: &Path,
    _log_path: &Path,
    child_command: &str,
) -> String {
    format!(
        "\"{}\" -c \"sxn ld; sxe 0xc0000142; sxe 0xc0000139; g; .lastevent; .exr -1; .ecxr; ln @$ip; lm a @$ip; q\" {}",
        cdb_path.display(),
        child_command
    )
}

pub(crate) fn select_poc_child_command(
    cdb_path: Option<&Path>,
    log_path: Option<&Path>,
    child_command: &str,
) -> PocResult<String> {
    match (cdb_path, log_path) {
        (None, None) => Ok(child_command.to_owned()),
        (Some(cdb), Some(log)) => Ok(build_cdb_child_command(cdb, log, child_command)),
        _ => Err("CDB executable and log path must be provided together".into()),
    }
}

pub(crate) fn validate_cdb_path(path: &Path) -> PocResult<PathBuf> {
    if !path.is_absolute() {
        return Err("CDB path must be absolute".into());
    }
    let expected_root = Path::new(r"C:\Program Files (x86)\Windows Kits\10\Debuggers\x64");
    let canonical = validate_cdb_path_with_meta(path, expected_root, |p| {
        let meta = std::fs::symlink_metadata(p)?;
        use std::os::windows::fs::MetadataExt;
        Ok(PocFileMetadata {
            is_symlink: meta.file_type().is_symlink(),
            is_reparse_point: meta.file_attributes() & 0x400 != 0,
        })
    })?;
    let metadata =
        std::fs::symlink_metadata(&canonical).map_err(|e| format!("cannot inspect CDB: {e}"))?;
    if metadata.file_type().is_symlink() {
        return Err("CDB path cannot be a symlink".into());
    }
    Ok(canonical)
}

pub(crate) fn validate_cdb_path_with_meta<F>(
    path: &Path,
    expected_root: &Path,
    meta_lookup: F,
) -> PocResult<PathBuf>
where
    F: FnMut(&Path) -> std::io::Result<PocFileMetadata>,
{
    let canonical = validate_poc_path_integrity_with_meta(path, expected_root, meta_lookup)
        .map_err(|e| format!("invalid CDB path: {e}"))?;
    let canonical_root = expected_root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize Windows Kits debugger directory: {e}"))?;
    validate_cdb_canonical_path(&canonical, &canonical_root)
}

const CDB_LOG_LIMIT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct CdbCaptureBuffer {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl Default for CdbCaptureBuffer {
    fn default() -> Self {
        Self {
            bytes: Vec::with_capacity(CDB_LOG_LIMIT_BYTES as usize),
            exceeded: false,
        }
    }
}

impl CdbCaptureBuffer {
    fn push(&mut self, chunk: &[u8]) {
        let remaining = CDB_LOG_LIMIT_BYTES as usize - self.bytes.len();
        let accepted = remaining.min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..accepted]);
        if accepted < chunk.len() {
            self.exceeded = true;
        }
    }
}

fn persist_cdb_capture(log_path: Option<&Path>, capture: &CdbCaptureBuffer) -> PocResult<()> {
    if let Some(path) = log_path {
        std::fs::write(path, &capture.bytes).map_err(|e| format_cdb_persist_error(&e))?;
    }
    Ok(())
}

fn format_cdb_persist_error(error: &std::io::Error) -> String {
    format!("cannot persist bounded CDB trace: {error}")
}

fn cdb_report_image_name(path: &Path) -> String {
    path.file_name()
        .map(OsStr::to_string_lossy)
        .map(|name| name.into_owned())
        .unwrap_or_else(|| "<unavailable>".to_string())
}

struct CdbPipeCapture {
    read: Option<PocHandleGuard>,
    write: Option<PocHandleGuard>,
    stdin: Option<PocHandleGuard>,
    attributes: PocProcThreadAttributeList,
}

struct PocProcThreadAttributeList {
    ptr: windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST,
    storage: Vec<usize>,
}

impl PocProcThreadAttributeList {
    fn for_handles(handles: &[HANDLE]) -> PocResult<Self> {
        if handles.is_empty() || handles.iter().any(|handle| handle.is_null()) {
            return Err("CDB handle allowlist is empty or contains a null handle".into());
        }
        let mut required = 0usize;
        unsafe {
            let _ = InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut required);
        }
        if required == 0 {
            return Err("InitializeProcThreadAttributeList returned an empty size".into());
        }
        let mut storage = vec![0usize; required.div_ceil(std::mem::size_of::<usize>())];
        let ptr = storage.as_mut_ptr().cast();
        if unsafe { InitializeProcThreadAttributeList(ptr, 1, 0, &mut required) } == 0 {
            return Err(format!(
                "InitializeProcThreadAttributeList failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        let value = Self { ptr, storage };
        let ok = unsafe {
            UpdateProcThreadAttribute(
                value.ptr,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast(),
                std::mem::size_of_val(handles),
                null_mut(),
                null(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "UpdateProcThreadAttribute(handle allowlist) failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        Ok(value)
    }
}

impl Drop for PocProcThreadAttributeList {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { DeleteProcThreadAttributeList(self.ptr) };
            self.ptr = null_mut();
        }
        let _ = &self.storage;
    }
}

impl CdbPipeCapture {
    fn new() -> PocResult<Self> {
        let mut security = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>()
                as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let mut read = null_mut();
        let mut write = null_mut();
        if unsafe { CreatePipe(&mut read, &mut write, &mut security, 0) } == 0 {
            return Err(format!("CreatePipe(CDB output) failed: error {}", unsafe {
                GetLastError()
            }));
        }
        let read = PocHandleGuard(read);
        let write = PocHandleGuard(write);
        if unsafe { SetHandleInformation(read.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(format!(
                "SetHandleInformation(CDB pipe reader) failed: error {}",
                unsafe { GetLastError() }
            ));
        }

        let nul_path = to_wide_null("NUL");
        let stdin = unsafe {
            CreateFileW(
                nul_path.as_ptr(),
                FILE_GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &mut security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if stdin == INVALID_HANDLE_VALUE || stdin.is_null() {
            return Err(format!(
                "CreateFileW(NUL for CDB stdin) failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        let stdin = PocHandleGuard(stdin);
        let attributes = PocProcThreadAttributeList::for_handles(&[stdin.0, write.0])?;
        Ok(Self {
            read: Some(read),
            write: Some(write),
            stdin: Some(stdin),
            attributes,
        })
    }
}

fn drain_cdb_pipe(read_handle: HANDLE, capture: &mut CdbCaptureBuffer) -> PocResult<()> {
    let mut available = 0u32;
    if unsafe {
        PeekNamedPipe(
            read_handle,
            null_mut(),
            0,
            null_mut(),
            &mut available,
            null_mut(),
        )
    } == 0
    {
        let error = unsafe { GetLastError() };
        if error == ERROR_BROKEN_PIPE {
            return Ok(());
        }
        return Err(format!("PeekNamedPipe(CDB output) failed: error {error}"));
    }
    while available > 0 {
        let mut chunk = [0u8; 8192];
        let requested = available.min(chunk.len() as u32);
        let mut read = 0u32;
        if unsafe {
            ReadFile(
                read_handle,
                chunk.as_mut_ptr(),
                requested,
                &mut read,
                null_mut(),
            )
        } == 0
        {
            let error = unsafe { GetLastError() };
            if error == ERROR_BROKEN_PIPE {
                return Ok(());
            }
            return Err(format!("ReadFile(CDB output pipe) failed: error {error}"));
        }
        if read == 0 {
            break;
        }
        capture.push(&chunk[..read as usize]);
        if unsafe {
            PeekNamedPipe(
                read_handle,
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            )
        } == 0
        {
            let error = unsafe { GetLastError() };
            if error == ERROR_BROKEN_PIPE {
                break;
            }
            return Err(format!("PeekNamedPipe(CDB output) failed: error {error}"));
        }
    }
    Ok(())
}

pub(crate) fn cdb_log_exceeds_limit(size: u64) -> bool {
    size > CDB_LOG_LIMIT_BYTES
}

pub(crate) fn validate_cdb_canonical_path(
    path: &Path,
    canonical_root: &Path,
) -> PocResult<PathBuf> {
    if !path.is_absolute() || !canonical_root.is_absolute() || !path.starts_with(canonical_root) {
        return Err("CDB path is outside the canonical Windows Kits debugger root".into());
    }
    if !path
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|n| n.eq_ignore_ascii_case("cdb.exe"))
    {
        return Err("CDB leaf must be cdb.exe".into());
    }
    Ok(path.to_path_buf())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CdbTraceClassification {
    DiagnosticNotStarted,
    ObservedDllInitFailedStatus,
    ObservedEntryPointNotFoundStatus,
    DebuggerFailed,
    TargetExitUnobserved,
    TargetExitedUnidentified,
    TraceTruncated,
    AclRestoreFailed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CdbTraceSummary {
    pub debugger_version_line: Option<String>,
    pub module_events: Vec<String>,
    pub observed_statuses: Vec<String>,
    pub target_exit_line: Option<String>,
    pub ordered_events: Vec<CdbTraceEvent>,
    pub truncated: bool,
}

#[derive(Debug)]
struct PendingCdbReport {
    run_id: String,
    target_image: Option<String>,
    debugger_exit: Option<u32>,
    classification: CdbTraceClassification,
    summary: CdbTraceSummary,
}

impl PendingCdbReport {
    fn new(run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            target_image: None,
            debugger_exit: None,
            classification: CdbTraceClassification::DiagnosticNotStarted,
            summary: CdbTraceSummary::default(),
        }
    }

    fn observe(
        &mut self,
        target_image: Option<&Path>,
        debugger_exit: Option<u32>,
        trace: &[u8],
        truncated: bool,
        fallback: CdbTraceClassification,
    ) {
        if let Some(target_image) = target_image {
            self.target_image = Some(cdb_report_image_name(target_image));
        }
        if debugger_exit.is_some() {
            self.debugger_exit = debugger_exit;
        }
        let trace = String::from_utf8_lossy(trace);
        self.summary = summarize_cdb_trace(&trace, truncated);
        self.classification = if truncated {
            CdbTraceClassification::TraceTruncated
        } else if trace.is_empty() {
            fallback
        } else {
            let parsed = classify_cdb_trace(&trace, false, false);
            if parsed == CdbTraceClassification::TargetExitedUnidentified
                && self.summary.target_exit_line.is_none()
            {
                CdbTraceClassification::TargetExitUnobserved
            } else {
                parsed
            }
        };
    }

    fn render(&self) -> String {
        format!(
            "[POC-CDB] run_id={} target_image={:?} debugger_exit={:?} classification={:?} summary={:?}",
            self.run_id, self.target_image, self.debugger_exit, self.classification, self.summary
        )
    }
}

fn apply_cdb_acl_restore_result(report: &mut Option<PendingCdbReport>, failed: bool) {
    if failed {
        if let Some(report) = report.as_mut() {
            report.classification = CdbTraceClassification::AclRestoreFailed;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CdbTraceEvent {
    pub sequence: u32,
    pub kind: &'static str,
    pub detail: String,
}

/// Extracts only bounded, low-sensitivity trace facts. Module paths are reduced
/// to basenames; raw debugger output is never copied into the returned summary.
fn sanitize_cdb_debugger_version_line(line: &str) -> Option<String> {
    const MAX_LINE_CHARS: usize = 240;
    let safe_prefix = line
        .split_whitespace()
        .take_while(|token| {
            !token.contains('\\')
                && !token.contains('/')
                && !token
                    .as_bytes()
                    .get(0..2)
                    .is_some_and(|prefix| prefix[0].is_ascii_alphabetic() && prefix[1] == b':')
        })
        .collect::<Vec<_>>()
        .join(" ");
    let bounded: String = safe_prefix.chars().take(MAX_LINE_CHARS).collect();
    (!bounded.is_empty()).then_some(bounded)
}

fn parse_cdb_exit_status_token(text: &str) -> Option<&str> {
    let token = text.split_whitespace().next()?;
    if let Some(hex) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        (hex.len() <= 8 && u32::from_str_radix(hex, 16).is_ok()).then_some(token)
    } else {
        (token.len() <= 10 && token.parse::<u32>().is_ok()).then_some(token)
    }
}

pub(crate) fn summarize_cdb_trace(log: &str, truncated: bool) -> CdbTraceSummary {
    const MAX_EVENTS: usize = 32;
    let mut summary = CdbTraceSummary {
        truncated,
        ..Default::default()
    };
    for line in log.lines() {
        let lower = line.to_ascii_lowercase();
        if summary.debugger_version_line.is_none()
            && lower.contains("debugger")
            && lower.contains("version")
        {
            summary.debugger_version_line = sanitize_cdb_debugger_version_line(line);
        }
        if lower.contains("modload:") && summary.module_events.len() < MAX_EVENTS {
            let leaf = line.split_whitespace().last().unwrap_or_default();
            let basename = Path::new(leaf)
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or(leaf);
            let basename: String = basename.chars().take(128).collect();
            summary.module_events.push(basename.clone());
            push_cdb_event(&mut summary, "module_loaded", basename);
        }
        for status in ["c0000142", "c0000139"] {
            if lower.contains(status)
                && summary.observed_statuses.len() < MAX_EVENTS
                && !summary.observed_statuses.iter().any(|v| v == status)
            {
                summary.observed_statuses.push(status.to_string());
                push_cdb_event(&mut summary, "exception_status", status.to_string());
            }
        }
        if summary.target_exit_line.is_none() && lower.contains("exited with code") {
            let code = lower
                .split_once("exited with code")
                .and_then(|(_, code)| parse_cdb_exit_status_token(code));
            if let Some(code) = code {
                summary.target_exit_line = Some(format!("target exited with code {code}"));
                let detail = summary.target_exit_line.clone().unwrap_or_default();
                push_cdb_event(&mut summary, "target_exit", detail);
            }
        }
    }
    summary
}

fn push_cdb_event(summary: &mut CdbTraceSummary, kind: &'static str, detail: String) {
    if summary.ordered_events.len() < 32 {
        summary.ordered_events.push(CdbTraceEvent {
            sequence: summary.ordered_events.len() as u32 + 1,
            kind,
            detail,
        });
    }
}

pub(crate) fn classify_cdb_trace(
    log: &str,
    truncated: bool,
    acl_restore_failed: bool,
) -> CdbTraceClassification {
    if acl_restore_failed {
        return CdbTraceClassification::AclRestoreFailed;
    }
    if truncated {
        return CdbTraceClassification::TraceTruncated;
    }
    let normalized = log.to_ascii_lowercase();
    if normalized.lines().any(|line| {
        line.contains("exception") && (line.contains("0xc0000142") || line.contains("c0000142"))
    }) {
        return CdbTraceClassification::ObservedDllInitFailedStatus;
    }
    if normalized.lines().any(|line| {
        line.contains("exception") && (line.contains("0xc0000139") || line.contains("c0000139"))
    }) {
        return CdbTraceClassification::ObservedEntryPointNotFoundStatus;
    }
    if normalized.contains("unable to start") || normalized.contains("failed to start") {
        return CdbTraceClassification::DebuggerFailed;
    }
    CdbTraceClassification::TargetExitedUnidentified
}

pub(crate) fn resolve_cdb_diagnostic_path(
    opt_in: Option<&str>,
    path: Option<&Path>,
) -> PocResult<Option<PathBuf>> {
    match (opt_in, path) {
        (None | Some("0"), None) => Ok(None),
        (Some("1"), Some(path)) => validate_cdb_path(path).map(Some),
        (Some("1"), None) => Err("CDB diagnostic opt-in requires POVERELLO_STAGE2_CDB_PATH".into()),
        (None | Some("0"), Some(_)) => {
            Err("CDB path supplied without exact diagnostic opt-in".into())
        }
        _ => Err("CDB diagnostic opt-in must be exactly 1 or unset/0".into()),
    }
}

fn build_poc_system_cmd_smoke_command(system_root: &Path) -> String {
    let cmd_exe = system_root.join("System32").join("cmd.exe");
    format!("\"{}\" /d /c exit 0", cmd_exe.display())
}

fn require_poc_native_startup_helper_path(path: Option<&Path>) -> PocResult<PathBuf> {
    let path = path.ok_or_else(|| {
        "missing POVERELLO_POC_STARTUP_HELPER_EXE; native probe refused before fixture creation"
            .to_string()
    })?;
    if !path.is_absolute() {
        return Err("POVERELLO_POC_STARTUP_HELPER_EXE must be an absolute path".into());
    }
    Ok(path.to_path_buf())
}

fn validate_poc_native_startup_helper_path(path: &Path, target_root: &Path) -> PocResult<PathBuf> {
    validate_poc_path_integrity(path, target_root)
        .map_err(|error| format!("invalid native startup helper path: {error}"))
}

fn require_poc_user32_startup_helper_path(path: Option<&Path>) -> PocResult<PathBuf> {
    let path = path.ok_or_else(|| {
        "missing POVERELLO_POC_USER32_STARTUP_HELPER_EXE; native probe refused before fixture creation"
            .to_string()
    })?;
    if !path.is_absolute() {
        return Err("POVERELLO_POC_USER32_STARTUP_HELPER_EXE must be an absolute path".into());
    }
    Ok(path.to_path_buf())
}

fn validate_poc_user32_startup_helper_path(path: &Path, target_root: &Path) -> PocResult<PathBuf> {
    validate_poc_path_integrity(path, target_root)
        .map_err(|error| format!("invalid USER32 startup helper path: {error}"))
}

fn require_poc_userenv_startup_helper_path(path: Option<&Path>) -> PocResult<PathBuf> {
    let path = path.ok_or_else(|| {
        "missing POVERELLO_POC_USERENV_STARTUP_HELPER_EXE; native probe refused before fixture creation"
            .to_string()
    })?;
    if !path.is_absolute() {
        return Err("POVERELLO_POC_USERENV_STARTUP_HELPER_EXE must be an absolute path".into());
    }
    Ok(path.to_path_buf())
}

fn validate_poc_userenv_startup_helper_path(path: &Path, target_root: &Path) -> PocResult<PathBuf> {
    validate_poc_path_integrity(path, target_root)
        .map_err(|error| format!("invalid USERENV startup helper path: {error}"))
}

fn require_poc_vcruntime_startup_helper_path(path: Option<&Path>) -> PocResult<PathBuf> {
    let path = path.ok_or_else(|| {
        "missing POVERELLO_POC_VCRUNTIME_STARTUP_HELPER_EXE; native probe refused before fixture creation"
            .to_string()
    })?;
    if !path.is_absolute() {
        return Err("POVERELLO_POC_VCRUNTIME_STARTUP_HELPER_EXE must be an absolute path".into());
    }
    Ok(path.to_path_buf())
}

fn validate_poc_vcruntime_startup_helper_path(
    path: &Path,
    target_root: &Path,
) -> PocResult<PathBuf> {
    validate_poc_path_integrity(path, target_root)
        .map_err(|error| format!("invalid VCRUNTIME startup helper path: {error}"))
}

fn require_poc_crt_runtime_startup_helper_path(path: Option<&Path>) -> PocResult<PathBuf> {
    let path = path.ok_or_else(|| {
        "missing POVERELLO_POC_CRT_RUNTIME_STARTUP_HELPER_EXE; native probe refused before fixture creation"
            .to_string()
    })?;
    if !path.is_absolute() {
        return Err("POVERELLO_POC_CRT_RUNTIME_STARTUP_HELPER_EXE must be an absolute path".into());
    }
    Ok(path.to_path_buf())
}

fn validate_poc_crt_runtime_startup_helper_path(
    path: &Path,
    target_root: &Path,
) -> PocResult<PathBuf> {
    validate_poc_path_integrity(path, target_root)
        .map_err(|error| format!("invalid CRT runtime startup helper path: {error}"))
}

fn require_poc_grouped_crt_startup_helper_path(path: Option<&Path>) -> PocResult<PathBuf> {
    let path = path.ok_or_else(|| {
        "missing POVERELLO_POC_GROUPED_CRT_STARTUP_HELPER_EXE; native probe refused before fixture creation"
            .to_string()
    })?;
    if !path.is_absolute() {
        return Err("POVERELLO_POC_GROUPED_CRT_STARTUP_HELPER_EXE must be an absolute path".into());
    }
    Ok(path.to_path_buf())
}

fn validate_poc_grouped_crt_startup_helper_path(
    path: &Path,
    target_root: &Path,
) -> PocResult<PathBuf> {
    validate_poc_path_integrity(path, target_root)
        .map_err(|error| format!("invalid grouped CRT startup helper path: {error}"))
}

fn build_poc_native_startup_helper_command(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn poc_child_environment_pairs(fixture: &PocFixture, controller_pid: u32) -> Vec<(String, String)> {
    vec![
        ("POVERELLO_POC_CHILD_ROLE".to_string(), "1".to_string()),
        (
            "POVERELLO_POC_CONTROLLER_PID".to_string(),
            controller_pid.to_string(),
        ),
        (
            "POVERELLO_POC_CANDIDATE_DIR".to_string(),
            fixture.candidate_dir.display().to_string(),
        ),
        (
            "POVERELLO_POC_SCRATCH_DIR".to_string(),
            fixture.scratch_dir.display().to_string(),
        ),
        (
            "POVERELLO_POC_PRIMARY_DIR".to_string(),
            fixture.primary_dir.display().to_string(),
        ),
        (
            "POVERELLO_POC_ENGINE_DIR".to_string(),
            fixture.engine_state_dir.display().to_string(),
        ),
        (
            "POVERELLO_POC_SENTINELS_DIR".to_string(),
            fixture.sentinels_dir.display().to_string(),
        ),
        (
            "POVERELLO_POC_OUTSIDE_DIR".to_string(),
            fixture.outside_dir.display().to_string(),
        ),
    ]
}

/// Launches a no-op system command with the same token, station, desktop,
/// environment, and Job Object as the Rust child probe. This distinguishes
/// general process initialization failures from test-harness-specific failures.
fn launch_poc_system_cmd_smoke_child(
    restricted_token: HANDLE,
    window_station_name: &str,
    desktop_name: &str,
    job: &PocJobObject,
    env_pairs: &[(String, String)],
) -> PocResult<u32> {
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or_else(|| "missing SystemRoot for cmd.exe startup smoke".to_string())?;
    let command_line = build_poc_system_cmd_smoke_command(&system_root);
    let mut command_line_w = to_wide_null(&command_line);
    let lp_desktop = build_poc_lp_desktop_name(window_station_name, desktop_name)?;
    let mut desktop_w = to_wide_null(&lp_desktop);
    let mut env_block = create_unicode_environment_block(env_pairs)?;

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.lpDesktop = desktop_w.as_mut_ptr();
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = 0;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let ok = unsafe {
        CreateProcessAsUserW(
            restricted_token,
            null(),
            command_line_w.as_mut_ptr(),
            null(),
            null(),
            FALSE,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
            env_block.as_mut_ptr() as _,
            null(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(format!(
            "startup-smoke CreateProcessAsUserW failed: error {}",
            unsafe { GetLastError() }
        ));
    }

    let process_guard = PocHandleGuard(pi.hProcess);
    let thread_guard = PocHandleGuard(pi.hThread);
    job.assign_process(pi.hProcess)
        .map_err(|e| format!("startup-smoke {e}"))?;

    if unsafe { ResumeThread(pi.hThread) } == u32::MAX {
        return Err(format!(
            "startup-smoke ResumeThread failed: error {}",
            unsafe { GetLastError() }
        ));
    }
    let wait_result = unsafe { WaitForSingleObject(pi.hProcess, 30_000) };
    if wait_result != 0 {
        return Err(format!(
            "startup-smoke child wait failed: result {wait_result}"
        ));
    }

    let mut exit_code = 0u32;
    if unsafe { GetExitCodeProcess(pi.hProcess, &mut exit_code) } == 0 {
        return Err(format!(
            "startup-smoke GetExitCodeProcess failed: error {}",
            unsafe { GetLastError() }
        ));
    }
    drop(thread_guard);
    drop(process_guard);
    if exit_code != 0 {
        return Err(format!(
            "startup-smoke cmd.exe exited nonzero: {}",
            format_poc_child_exit_status(exit_code)
        ));
    }
    Ok(exit_code)
}

/// Launches the CRT-free diagnostic helper with the exact restricted startup context.
fn launch_poc_native_startup_helper(
    restricted_token: HANDLE,
    helper_path: &Path,
    window_station_name: &str,
    desktop_name: &str,
    job: &PocJobObject,
    env_pairs: &[(String, String)],
) -> PocResult<u32> {
    let mut application_w = to_wide_null(helper_path);
    let command_line = build_poc_native_startup_helper_command(helper_path);
    let mut command_line_w = to_wide_null(&command_line);
    let lp_desktop = build_poc_lp_desktop_name(window_station_name, desktop_name)?;
    let mut desktop_w = to_wide_null(&lp_desktop);
    let mut env_block = create_unicode_environment_block(env_pairs)?;

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.lpDesktop = desktop_w.as_mut_ptr();
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = 0;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let ok = unsafe {
        CreateProcessAsUserW(
            restricted_token,
            application_w.as_mut_ptr(),
            command_line_w.as_mut_ptr(),
            null(),
            null(),
            FALSE,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
            env_block.as_mut_ptr() as _,
            null(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(format!(
            "native startup helper CreateProcessAsUserW failed: error {}",
            unsafe { GetLastError() }
        ));
    }

    let process_guard = PocHandleGuard(pi.hProcess);
    let thread_guard = PocHandleGuard(pi.hThread);
    if let Err(error) = job.assign_process(pi.hProcess) {
        let terminate_result = unsafe { TerminateProcess(pi.hProcess, 1) };
        let terminate_error = if terminate_result == 0 {
            format!("; TerminateProcess failed: error {}", unsafe {
                GetLastError()
            })
        } else {
            String::new()
        };
        let wait_result = unsafe { WaitForSingleObject(pi.hProcess, 30_000) };
        return Err(format!(
            "native startup helper {error}; suspended child termination wait result {wait_result}{terminate_error}"
        ));
    }
    if unsafe { ResumeThread(pi.hThread) } == u32::MAX {
        return Err(format!(
            "native startup helper ResumeThread failed: error {}",
            unsafe { GetLastError() }
        ));
    }
    let wait_result = unsafe { WaitForSingleObject(pi.hProcess, 30_000) };
    if wait_result != 0 {
        return Err(format!(
            "native startup helper wait failed: result {wait_result}"
        ));
    }

    let mut exit_code = 0u32;
    if unsafe { GetExitCodeProcess(pi.hProcess, &mut exit_code) } == 0 {
        return Err(format!(
            "native startup helper GetExitCodeProcess failed: error {}",
            unsafe { GetLastError() }
        ));
    }
    drop(thread_guard);
    drop(process_guard);
    if exit_code != 0 {
        return Err(format!(
            "native startup helper exited nonzero: {}",
            format_poc_child_exit_status(exit_code)
        ));
    }
    Ok(exit_code)
}

/// Builds an explicit `window-station\\desktop` STARTUPINFO name.
/// Each component must be a single non-empty object name.
fn build_poc_lp_desktop_name(window_station_name: &str, desktop_name: &str) -> PocResult<String> {
    let valid_component =
        |value: &str| !value.is_empty() && !value.chars().any(|ch| matches!(ch, '\\' | '/' | '\0'));
    if !valid_component(window_station_name) || !valid_component(desktop_name) {
        return Err("invalid Window Station or Desktop name component".into());
    }
    Ok(format!("{window_station_name}\\{desktop_name}"))
}

const STATUS_DLL_INIT_FAILED: u32 = 0xC000_0142;

/// Formats a Windows child exit status without losing the raw decimal value.
fn format_poc_child_exit_status(exit_code: u32) -> String {
    let symbolic = if exit_code == STATUS_DLL_INIT_FAILED {
        " status=STATUS_DLL_INIT_FAILED"
    } else {
        ""
    };
    format!("decimal={exit_code} hex=0x{exit_code:08X}{symbolic}")
}

/// Pure derivation of 4 subauthorities from UUID bytes for the restriction SID.
pub(crate) fn derive_restriction_subauthorities(uuid: uuid::Uuid) -> [u32; 4] {
    let bytes = uuid.as_bytes();
    let s0 = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let s1 = u32::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let s2 = u32::from_ne_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let s3 = u32::from_ne_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    [s0, s1, s2, s3]
}

/// Converts an `&OsStr` or `&Path` to a null-terminated wide string `Vec<u16>`.
pub(crate) fn to_wide_null(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

/// Creates a Windows Unicode environment block:
/// - Enforces ASCII keys.
/// - Validates keys (non-empty, no '=', no NUL).
/// - Rejects duplicate case-insensitive keys.
/// - Sorts keys in ASCII case-insensitive order.
/// - Encodes as null-separated null-terminated wide strings with double-NUL terminator.
pub(crate) fn create_unicode_environment_block(pairs: &[(String, String)]) -> PocResult<Vec<u16>> {
    let mut seen_keys = std::collections::HashSet::new();
    let mut sorted_pairs = Vec::with_capacity(pairs.len());

    for (k, v) in pairs {
        if k.is_empty() {
            return Err("environment variable name cannot be empty".to_string());
        }
        if !k.is_ascii() {
            return Err(format!("environment variable name '{k}' must be ASCII"));
        }
        if k.contains('=') {
            return Err(format!(
                "environment variable name '{k}' cannot contain '='"
            ));
        }
        if k.contains('\0') || v.contains('\0') {
            return Err("environment variable key or value cannot contain NUL".to_string());
        }
        let upper = k.to_ascii_uppercase();
        if !seen_keys.insert(upper) {
            return Err(format!(
                "duplicate case-insensitive environment variable: '{k}'"
            ));
        }
        sorted_pairs.push((k.clone(), v.clone()));
    }

    sorted_pairs.sort_by(|a, b| a.0.to_ascii_uppercase().cmp(&b.0.to_ascii_uppercase()));

    let mut block = Vec::new();
    for (k, v) in sorted_pairs {
        let entry = format!("{k}={v}");
        for c in OsStr::new(&entry).encode_wide() {
            block.push(c);
        }
        block.push(0);
    }
    block.push(0); // double-null terminated
    Ok(block)
}

/// Typed outcome of a filesystem or process security probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OperationOutcome {
    Allowed,
    DeniedAccessDenied, // exactly raw OS error 5 (ERROR_ACCESS_DENIED)
    FailedOther(i32),   // any other OS code or synthetic error
    NotAttempted,
}

impl Default for OperationOutcome {
    fn default() -> Self {
        OperationOutcome::NotAttempted
    }
}

impl OperationOutcome {
    pub(crate) fn is_denied(&self) -> bool {
        matches!(self, OperationOutcome::DeniedAccessDenied)
    }

    pub(crate) fn from_io_result<T>(res: &std::io::Result<T>) -> Self {
        match res {
            Ok(_) => OperationOutcome::Allowed,
            Err(e) => {
                if e.raw_os_error() == Some(5) {
                    OperationOutcome::DeniedAccessDenied
                } else {
                    OperationOutcome::FailedOther(e.raw_os_error().unwrap_or(-1))
                }
            }
        }
    }
}

/// Pure producer mapping from OpenProcess outcome and optional OpenProcessToken result to OperationOutcome.
/// Enforces §153.2 requirement 5:
/// - If OpenProcess failed, OpenProcessToken was never attempted; returns `NotAttempted`.
/// - If OpenProcess succeeded, OpenProcessToken must have been attempted:
///   - Error 5 is classified strictly as `DeniedAccessDenied`.
///   - Any other OS error code is classified as `FailedOther(code)`.
///   - Success is classified as `Allowed`.
///   - Missing token attempt returns `NotAttempted`.
pub(crate) fn map_process_and_token_open_outcome(
    open_process_result: Result<(), u32>,
    open_token_result: Option<Result<(), u32>>,
) -> OperationOutcome {
    match open_process_result {
        Err(_) => OperationOutcome::NotAttempted,
        Ok(()) => match open_token_result {
            None => OperationOutcome::NotAttempted,
            Some(Ok(())) => OperationOutcome::Allowed,
            Some(Err(5)) => OperationOutcome::DeniedAccessDenied,
            Some(Err(code)) => OperationOutcome::FailedOther(code as i32),
        },
    }
}

/// Maps the actual prerequisite and token-open results to evidence. A token denial is
/// accepted only when OpenProcessToken was attempted and returned raw error 5.
#[allow(dead_code)]
fn map_token_duplicate_result(
    process_open: OperationOutcome,
    token_error: Option<i32>,
) -> OperationOutcome {
    let proc_res = if process_open == OperationOutcome::Allowed {
        Ok(())
    } else {
        Err(1)
    };
    let tok_res = match token_error {
        None => Some(Ok(())),
        Some(err) => Some(Err(err as u32)),
    };
    map_process_and_token_open_outcome(proc_res, tok_res)
}

/// Aligned memory buffer for Win32 API structures requiring 8-byte alignment.
pub(crate) struct AlignedBuffer {
    data: Vec<u64>,
}

impl AlignedBuffer {
    pub(crate) fn new(byte_len: usize) -> Self {
        let u64_len = (byte_len + 7) / 8;
        Self {
            data: vec![0u64; u64_len.max(1)],
        }
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr() as *mut u8
    }

    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr() as *const u8
    }

    pub(crate) fn byte_capacity(&self) -> usize {
        self.data.len() * 8
    }
}

/// RAII wrapper for a Win32 `HANDLE`.
#[derive(Debug)]
pub(crate) struct PocHandleGuard(pub HANDLE);

impl Drop for PocHandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
            self.0 = null_mut();
        }
    }
}

/// RAII wrapper for an allocated Win32 `PSID`.
#[derive(Debug)]
pub(crate) struct PocSidGuard(pub PSID);

impl Drop for PocSidGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                FreeSid(self.0);
            }
            self.0 = null_mut();
        }
    }
}

/// RAII wrapper for an allocated Win32 `*mut ACL`.
#[derive(Debug)]
pub(crate) struct PocAclGuard(pub *mut ACL);

impl Drop for PocAclGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0 as _);
            }
            self.0 = null_mut();
        }
    }
}

/// RAII wrapper for a Win32 `HDESK`.
#[derive(Debug)]
pub(crate) struct PocDesktopGuard {
    pub handle: HDESK,
    pub name: String,
}

impl Drop for PocDesktopGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseDesktop(self.handle);
            }
            self.handle = null_mut();
        }
    }
}

/// Unique per-run restriction SID derived from a UUID using non-privileged authority.
pub(crate) struct PocRestrictionSid {
    pub sid: PocSidGuard,
    pub subauthorities: [u32; 4],
    pub string_representation: String,
}

/// Read-only observations of explicit ACEs for this run's restricting SID on the
/// process Window Station. These ACE masks are diagnostic inputs, not an effective
/// access-check result: inherited/group ACEs and the complete token access check
/// are intentionally not inferred here.
#[derive(Debug, Default)]
struct PocWindowStationSidAceEvidence {
    station_name: String,
    allow_masks: Vec<u32>,
    deny_masks: Vec<u32>,
    dacl_is_null: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PocWindowStationDaclSnapshot {
    dacl_present: bool,
    dacl_is_null: bool,
    dacl_defaulted: bool,
    dacl_protected: bool,
    dacl_bytes_in_use: Vec<u8>,
}

/// Only grants the restricted child the station rights needed to connect to
/// and use the already-created desktop. It deliberately omits desktop creation,
/// station attribute writes, screen reads, and logoff/shutdown rights.
const fn poc_window_station_child_rights() -> u32 {
    // WINSTA_ENUMDESKTOPS (0x0001) | WINSTA_READATTRIBUTES (0x0002) |
    // WINSTA_ACCESSCLIPBOARD (0x0004) | WINSTA_ACCESSGLOBALATOMS (0x0020) |
    // WINSTA_ENUMERATE (0x0100).
    0x0127
}

struct PocWindowStationHandleGuard(*mut std::ffi::c_void);

impl Drop for PocWindowStationHandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseWindowStation(self.0);
            }
            self.0 = null_mut();
        }
    }
}

/// Best-effort temporary per-run ACE mutation on the current Window Station.
/// Snapshot checks detect some DACL changes, but GetSecurityInfo/SetSecurityInfo
/// are not an atomic conditional update: a concurrent external DACL write can be
/// overwritten between those calls. This guard is diagnostic-only and must not
/// be used as a production-safe shared-Window-Station mutation mechanism.
struct PocWindowStationDaclGuard {
    station: PocWindowStationHandleGuard,
    station_name: String,
    restriction_sid: PSID,
    baseline: PocWindowStationDaclSnapshot,
    mutation_attempted: bool,
    restore_attempted: bool,
    restored: bool,
}

impl PocWindowStationDaclGuard {
    fn capture_current(restriction_sid: &PocRestrictionSid) -> PocResult<Self> {
        let process_station = unsafe { GetProcessWindowStation() };
        if process_station.is_null() {
            return Err(format!(
                "GetProcessWindowStation failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        let mut needed = 0u32;
        unsafe {
            GetUserObjectInformationW(process_station as _, UOI_NAME, null_mut(), 0, &mut needed);
        }
        if needed < 2 || needed % 2 != 0 {
            return Err(format!("invalid Window Station name size: {needed}"));
        }
        let mut name_buf = vec![0u16; (needed / 2) as usize];
        let ok = unsafe {
            GetUserObjectInformationW(
                process_station as _,
                UOI_NAME,
                name_buf.as_mut_ptr() as _,
                needed,
                &mut needed,
            )
        };
        if ok == 0 {
            return Err(format!(
                "GetUserObjectInformationW(UOI_NAME) failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        let name_end = name_buf
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(name_buf.len());
        let station_name = String::from_utf16_lossy(&name_buf[..name_end]);
        if station_name.is_empty() {
            return Err("current Window Station name is empty".into());
        }

        // READ_CONTROL | WRITE_DAC are required to snapshot and update the DACL.
        let station_name_w = to_wide_null(&station_name);
        let station = unsafe {
            OpenWindowStationW(
                station_name_w.as_ptr(),
                FALSE,
                windows_sys::Win32::Storage::FileSystem::READ_CONTROL
                    | windows_sys::Win32::Storage::FileSystem::WRITE_DAC,
            )
        };
        if station.is_null() {
            return Err(format!(
                "OpenWindowStationW('{station_name}', READ_CONTROL | WRITE_DAC) failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        let station = PocWindowStationHandleGuard(station);
        let baseline = read_window_station_dacl_snapshot(station.0)?;

        Ok(Self {
            station,
            station_name,
            restriction_sid: restriction_sid.sid.0,
            baseline,
            mutation_attempted: false,
            restore_attempted: false,
            restored: false,
        })
    }

    fn grant_child_station_access(&mut self) -> PocResult<()> {
        // An absent or NULL DACL already grants access; do not mutate it.
        if !self.baseline.dacl_present || self.baseline.dacl_is_null {
            self.restored = true;
            return Ok(());
        }
        if self.baseline.dacl_defaulted {
            return Err(
                "refusing Window Station DACL mutation because its DACL is defaulted".into(),
            );
        }

        let before = read_window_station_dacl_snapshot(self.station.0)?;
        if before != self.baseline {
            return Err(
                "Window Station DACL changed between snapshot and mutation; refusing to overwrite it"
                    .into(),
            );
        }
        let mut dacl: *mut ACL = null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        let status = unsafe {
            GetSecurityInfo(
                self.station.0 as _,
                SE_WINDOW_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS || descriptor.is_null() || dacl.is_null() {
            if !descriptor.is_null() {
                let _ = unsafe { LocalFree(descriptor as _) };
            }
            return Err(format!(
                "GetSecurityInfo(Window Station DACL before grant) failed: {status}"
            ));
        }
        let _descriptor_guard = PocAclGuard(descriptor as *mut ACL);
        let masks = unsafe {
            inspect_window_station_handle_sid_aces(
                self.station.0,
                &self.station_name,
                self.restriction_sid,
            )?
        };
        if !masks.allow_masks.is_empty() || !masks.deny_masks.is_empty() {
            return Err(format!(
                "unique restricting SID already has Window Station ACEs: allow={:?}, deny={:?}",
                masks.allow_masks, masks.deny_masks
            ));
        }

        let mut entry: EXPLICIT_ACCESS_W = unsafe { std::mem::zeroed() };
        entry.grfAccessPermissions = poc_window_station_child_rights();
        entry.grfAccessMode = GRANT_ACCESS;
        entry.grfInheritance = 0;
        entry.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        entry.Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
        entry.Trustee.ptstrName = self.restriction_sid as _;

        let mut new_dacl: *mut ACL = null_mut();
        let set_ret = unsafe { SetEntriesInAclW(1, &entry, dacl, &mut new_dacl) };
        if set_ret != ERROR_SUCCESS || new_dacl.is_null() {
            if !new_dacl.is_null() {
                let _ = unsafe { LocalFree(new_dacl as _) };
            }
            return Err(format!(
                "SetEntriesInAclW(Window Station grant) failed: {set_ret}"
            ));
        }
        let _new_dacl_guard = PocAclGuard(new_dacl);
        // Set before calling the mutator: even an ambiguous failure must trigger restoration.
        self.mutation_attempted = true;
        let set_status = unsafe {
            SetSecurityInfo(
                self.station.0 as _,
                SE_WINDOW_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                new_dacl,
                null_mut(),
            )
        };
        if set_status != ERROR_SUCCESS {
            return Err(format!(
                "SetSecurityInfo(Window Station grant) failed: {set_status}"
            ));
        }

        let after = unsafe {
            inspect_window_station_handle_sid_aces(
                self.station.0,
                &self.station_name,
                self.restriction_sid,
            )?
        };
        if after.deny_masks.len() != 0
            || !after.allow_masks.iter().any(|mask| {
                mask & poc_window_station_child_rights() == poc_window_station_child_rights()
            })
        {
            return Err(format!(
                "Window Station restricting-SID grant was not observed: allow={:?}, deny={:?}",
                after.allow_masks, after.deny_masks
            ));
        }
        Ok(())
    }

    fn restore(&mut self) -> PocResult<()> {
        if self.restore_attempted {
            return if self.restored {
                Ok(())
            } else {
                Err("Window Station DACL restoration was already attempted and failed".into())
            };
        }
        self.restore_attempted = true;
        if !self.mutation_attempted {
            self.restored = true;
            return Ok(());
        }

        let mut current_dacl: *mut ACL = null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        let status = unsafe {
            GetSecurityInfo(
                self.station.0 as _,
                SE_WINDOW_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut current_dacl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS || descriptor.is_null() || current_dacl.is_null() {
            if !descriptor.is_null() {
                let _ = unsafe { LocalFree(descriptor as _) };
            }
            return Err(format!(
                "GetSecurityInfo(Window Station restore) failed: {status}"
            ));
        }
        let _descriptor_guard = PocAclGuard(descriptor as *mut ACL);
        let mut entry: EXPLICIT_ACCESS_W = unsafe { std::mem::zeroed() };
        entry.grfAccessMode = REVOKE_ACCESS;
        entry.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        entry.Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
        entry.Trustee.ptstrName = self.restriction_sid as _;
        let mut restored_dacl: *mut ACL = null_mut();
        let set_ret = unsafe { SetEntriesInAclW(1, &entry, current_dacl, &mut restored_dacl) };
        if set_ret != ERROR_SUCCESS || restored_dacl.is_null() {
            if !restored_dacl.is_null() {
                let _ = unsafe { LocalFree(restored_dacl as _) };
            }
            return Err(format!(
                "SetEntriesInAclW(Window Station revoke) failed: {set_ret}"
            ));
        }
        let _restored_dacl_guard = PocAclGuard(restored_dacl);
        let set_status = unsafe {
            SetSecurityInfo(
                self.station.0 as _,
                SE_WINDOW_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                restored_dacl,
                null_mut(),
            )
        };
        if set_status != ERROR_SUCCESS {
            return Err(format!(
                "SetSecurityInfo(Window Station restore) failed: {set_status}"
            ));
        }

        let restored = read_window_station_dacl_snapshot(self.station.0)?;
        if restored != self.baseline {
            return Err(format!(
                "Window Station DACL did not return to its captured baseline (station '{}'); refusing to overwrite concurrent changes",
                self.station_name
            ));
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for PocWindowStationDaclGuard {
    fn drop(&mut self) {
        if self.mutation_attempted && !self.restore_attempted {
            if let Err(error) = self.restore() {
                eprintln!(
                    "[POC-WINDOW-STATION] emergency_restore=FAILED station={:?} error={error}",
                    self.station_name
                );
            }
        }
    }
}

fn read_window_station_dacl_snapshot(
    station: *mut std::ffi::c_void,
) -> PocResult<PocWindowStationDaclSnapshot> {
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let status = unsafe {
        GetSecurityInfo(
            station as _,
            SE_WINDOW_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS || descriptor.is_null() {
        if !descriptor.is_null() {
            let _ = unsafe { LocalFree(descriptor as _) };
        }
        return Err(format!(
            "GetSecurityInfo(Window Station snapshot) failed: {status}"
        ));
    }
    let _descriptor_guard = PocAclGuard(descriptor as *mut ACL);
    let mut present = 0;
    let mut defaulted = 0;
    if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) }
        == 0
    {
        return Err("GetSecurityDescriptorDacl(Window Station) failed".into());
    }
    let mut control = 0u16;
    let mut revision = 0u32;
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
        return Err("GetSecurityDescriptorControl(Window Station) failed".into());
    }
    let mut bytes = Vec::new();
    if present != 0 && !dacl.is_null() {
        let mut info: ACL_SIZE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            GetAclInformation(
                dacl,
                &mut info as *mut _ as _,
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            return Err(format!(
                "GetAclInformation(Window Station snapshot) failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        bytes = unsafe {
            std::slice::from_raw_parts(dacl as *const u8, info.AclBytesInUse as usize).to_vec()
        };
    }
    Ok(PocWindowStationDaclSnapshot {
        dacl_present: present != 0,
        dacl_is_null: present != 0 && dacl.is_null(),
        dacl_defaulted: defaulted != 0,
        dacl_protected: control & SE_DACL_PROTECTED != 0,
        dacl_bytes_in_use: bytes,
    })
}

/// Queries the current process Window Station name and DACL without changing either.
/// Failure is reported to the caller so the diagnostic can be marked unavailable
/// without changing the native probe's launch behavior.
unsafe fn inspect_window_station_sid_aces(
    restriction_sid: &PocRestrictionSid,
) -> PocResult<PocWindowStationSidAceEvidence> {
    let station = GetProcessWindowStation();
    if station.is_null() {
        return Err(format!(
            "GetProcessWindowStation failed: error {}",
            GetLastError()
        ));
    }

    let mut needed = 0u32;
    GetUserObjectInformationW(station as _, UOI_NAME, null_mut(), 0, &mut needed);
    if needed < 2 || needed % 2 != 0 {
        return Err(format!(
            "GetUserObjectInformationW(UOI_NAME) returned invalid size {needed}"
        ));
    }
    let mut name = vec![0u16; (needed / 2) as usize];
    if GetUserObjectInformationW(
        station as _,
        UOI_NAME,
        name.as_mut_ptr() as _,
        needed,
        &mut needed,
    ) == 0
    {
        return Err(format!(
            "GetUserObjectInformationW(UOI_NAME) failed: error {}",
            GetLastError()
        ));
    }
    let station_name = String::from_utf16_lossy(
        &name[..name
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(name.len())],
    );

    inspect_window_station_handle_sid_aces(station, &station_name, restriction_sid.sid.0)
}

/// Enumerates ordinary allow/deny ACEs for exactly `restriction_sid` on an
/// already-open Window Station. This is not an effective-access calculation.
unsafe fn inspect_window_station_handle_sid_aces(
    station: *mut std::ffi::c_void,
    station_name: &str,
    restriction_sid: PSID,
) -> PocResult<PocWindowStationSidAceEvidence> {
    if station.is_null() || restriction_sid.is_null() {
        return Err("Window Station ACE inspection received a null handle or SID".into());
    }

    let mut dacl: *mut ACL = null_mut();
    let mut security_descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let status = GetSecurityInfo(
        station as _,
        SE_WINDOW_OBJECT,
        DACL_SECURITY_INFORMATION,
        null_mut(),
        null_mut(),
        &mut dacl,
        null_mut(),
        &mut security_descriptor,
    );
    if status != ERROR_SUCCESS {
        return Err(format!("GetSecurityInfo(Window Station) failed: {status}"));
    }
    let _descriptor_guard = PocAclGuard(security_descriptor as *mut ACL);

    let mut evidence = PocWindowStationSidAceEvidence {
        station_name: station_name.to_owned(),
        dacl_is_null: dacl.is_null(),
        ..Default::default()
    };
    if dacl.is_null() {
        return Ok(evidence);
    }

    let mut acl_info: ACL_SIZE_INFORMATION = std::mem::zeroed();
    if GetAclInformation(
        dacl,
        &mut acl_info as *mut _ as _,
        std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
        AclSizeInformation,
    ) == 0
    {
        return Err(format!(
            "GetAclInformation(Window Station DACL) failed: error {}",
            GetLastError()
        ));
    }

    for index in 0..acl_info.AceCount {
        let mut ace_ptr: *mut std::ffi::c_void = null_mut();
        if GetAce(dacl, index, &mut ace_ptr) == 0 || ace_ptr.is_null() {
            return Err(format!(
                "GetAce(Window Station DACL, {index}) failed: error {}",
                GetLastError()
            ));
        }
        let header = &*(ace_ptr as *const ACE_HEADER);
        match header.AceType {
            x if x as u32 == ACCESS_ALLOWED_ACE_TYPE => {
                let ace = &*(ace_ptr as *const ACCESS_ALLOWED_ACE);
                let sid = std::ptr::addr_of!(ace.SidStart) as PSID;
                if EqualSid(sid, restriction_sid) != 0 {
                    evidence.allow_masks.push(ace.Mask);
                }
            }
            x if x as u32 == ACCESS_DENIED_ACE_TYPE => {
                let ace = &*(ace_ptr as *const ACCESS_DENIED_ACE);
                let sid = std::ptr::addr_of!(ace.SidStart) as PSID;
                if EqualSid(sid, restriction_sid) != 0 {
                    evidence.deny_masks.push(ace.Mask);
                }
            }
            _ => {}
        }
    }

    Ok(evidence)
}

impl PocRestrictionSid {
    pub(crate) fn new(uuid: uuid::Uuid) -> PocResult<Self> {
        let subs = derive_restriction_subauthorities(uuid);
        let mut auth = windows_sys::Win32::Security::SID_IDENTIFIER_AUTHORITY {
            Value: [0, 0, 0, 0, 0, 9], // SECURITY_RESOURCE_MANAGER_AUTHORITY
        };

        let mut psid: PSID = null_mut();
        let ok = unsafe {
            AllocateAndInitializeSid(
                &mut auth, 4, subs[0], subs[1], subs[2], subs[3], 0, 0, 0, 0, &mut psid,
            )
        };
        if ok == 0 || psid.is_null() {
            let err = unsafe { GetLastError() };
            return Err(format!("AllocateAndInitializeSid failed: error {err}"));
        }

        let string_rep = format!("S-1-9-{}-{}-{}-{}", subs[0], subs[1], subs[2], subs[3]);
        Ok(Self {
            sid: PocSidGuard(psid),
            subauthorities: subs,
            string_representation: string_rep,
        })
    }
}

/// Creates a dedicated non-default desktop with documented least-privilege access:
/// - Restriction SID: DESKTOP_CHILD_RIGHTS (0x00C3)
/// - Controller User SID: DESKTOP_CONTROLLER_RIGHTS (0x000600C3)
/// - SYSTEM SID: DESKTOP_SYSTEM_RIGHTS (0x00020041)
pub(crate) fn create_poc_isolated_desktop(
    uuid: uuid::Uuid,
    restriction_sid: &PocRestrictionSid,
) -> PocResult<PocDesktopGuard> {
    let desktop_name = format!("Poverello-PoC-Desktop-{uuid}");
    let desktop_name_w = to_wide_null(&desktop_name);

    let hdesk = unsafe {
        CreateDesktopW(
            desktop_name_w.as_ptr(),
            null(),
            null(),
            0,
            DESKTOP_CONTROLLER_RIGHTS,
            null(),
        )
    };
    if hdesk.is_null() {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "CreateDesktopW failed for '{desktop_name}': error {err}"
        ));
    }
    let guard = PocDesktopGuard {
        handle: hdesk,
        name: desktop_name,
    };

    // Obtain current process user SID
    let mut current_token: HANDLE = null_mut();
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut current_token) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "OpenProcessToken failed for desktop DACL setup: error {err}"
        ));
    }
    let token_guard = PocHandleGuard(current_token);

    let mut user_len: u32 = 0;
    unsafe {
        GetTokenInformation(current_token, TokenUser, null_mut(), 0, &mut user_len);
    }
    if user_len == 0 {
        return Err("GetTokenInformation for TokenUser returned 0 length".into());
    }
    let mut user_buf = AlignedBuffer::new(user_len as usize);
    let ok = unsafe {
        GetTokenInformation(
            current_token,
            TokenUser,
            user_buf.as_mut_ptr() as _,
            user_buf.byte_capacity() as u32,
            &mut user_len,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "GetTokenInformation for TokenUser failed: error {err}"
        ));
    }
    drop(token_guard);
    let token_user = user_buf.as_ptr() as *const TOKEN_USER;
    let user_sid = unsafe { (*token_user).User.Sid };

    // Allocate well-known SYSTEM SID (S-1-5-18)
    let mut nt_auth = windows_sys::Win32::Security::SID_IDENTIFIER_AUTHORITY {
        Value: [0, 0, 0, 0, 0, 5], // SECURITY_NT_AUTHORITY
    };
    let mut system_psid: PSID = null_mut();
    const SECURITY_LOCAL_SYSTEM_RID: u32 = 0x00000012;
    let ok = unsafe {
        AllocateAndInitializeSid(
            &mut nt_auth,
            1,
            SECURITY_LOCAL_SYSTEM_RID,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut system_psid,
        )
    };
    if ok == 0 || system_psid.is_null() {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "AllocateAndInitializeSid for SYSTEM failed: error {err}"
        ));
    }
    let _system_sid_guard = PocSidGuard(system_psid);

    // Build explicit ACE list with 3 least-privilege trustees
    let mut ea: [EXPLICIT_ACCESS_W; 3] = unsafe { std::mem::zeroed() };

    // ACE 0: Restriction SID (child rights only)
    ea[0].grfAccessPermissions = DESKTOP_CHILD_RIGHTS;
    ea[0].grfAccessMode = GRANT_ACCESS;
    ea[0].grfInheritance = 0;
    ea[0].Trustee.TrusteeForm = TRUSTEE_IS_SID;
    ea[0].Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
    ea[0].Trustee.ptstrName = restriction_sid.sid.0 as _;

    // ACE 1: Current Controller User SID (controller rights)
    ea[1].grfAccessPermissions = DESKTOP_CONTROLLER_RIGHTS;
    ea[1].grfAccessMode = GRANT_ACCESS;
    ea[1].grfInheritance = 0;
    ea[1].Trustee.TrusteeForm = TRUSTEE_IS_SID;
    ea[1].Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
    ea[1].Trustee.ptstrName = user_sid as _;

    // ACE 2: SYSTEM SID (minimal system rights)
    ea[2].grfAccessPermissions = DESKTOP_SYSTEM_RIGHTS;
    ea[2].grfAccessMode = GRANT_ACCESS;
    ea[2].grfInheritance = 0;
    ea[2].Trustee.TrusteeForm = TRUSTEE_IS_SID;
    ea[2].Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
    ea[2].Trustee.ptstrName = system_psid as _;

    let mut new_dacl: *mut ACL = null_mut();
    let set_ret = unsafe { SetEntriesInAclW(3, ea.as_ptr(), null_mut(), &mut new_dacl) };
    if set_ret != ERROR_SUCCESS {
        return Err(format!(
            "SetEntriesInAclW failed for desktop: error {set_ret}"
        ));
    }
    let _dacl_guard = PocAclGuard(new_dacl);

    let sec_ret = unsafe {
        SetSecurityInfo(
            guard.handle as _,
            SE_WINDOW_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_dacl,
            null_mut(),
        )
    };
    if sec_ret != ERROR_SUCCESS {
        return Err(format!(
            "SetSecurityInfo failed for isolated desktop: error {sec_ret}"
        ));
    }

    Ok(guard)
}

/// Job Object wrapper for descendant process lifecycle accounting and termination.
pub(crate) struct PocJobObject {
    #[allow(dead_code)]
    pub handle: PocHandleGuard,
    pub name: String,
}

fn wait_for_job_empty(job_handle: HANDLE, timeout: std::time::Duration) -> PocResult<()> {
    let started = std::time::Instant::now();
    loop {
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            QueryInformationJobObject(
                job_handle,
                JobObjectBasicAccountingInformation,
                &mut accounting as *mut _ as _,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "QueryInformationJobObject(active processes) failed: error {}",
                unsafe { GetLastError() }
            ));
        }
        if accounting.ActiveProcesses == 0 {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            return Err(format!(
                "PoC Job still has {} active process(es) after {:?}",
                accounting.ActiveProcesses, timeout
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

impl PocJobObject {
    pub(crate) fn new(uuid: uuid::Uuid) -> PocResult<Self> {
        let job_name = format!("Poverello-PoC-Job-{uuid}");
        let job_name_w = to_wide_null(&job_name);

        let hjob = unsafe { CreateJobObjectW(null(), job_name_w.as_ptr()) };
        if hjob.is_null() {
            let err = unsafe { GetLastError() };
            return Err(format!("CreateJobObjectW failed: error {err}"));
        }
        let guard = PocHandleGuard(hjob);

        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;

        let ok = unsafe {
            SetInformationJobObject(
                hjob,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            return Err(format!("SetInformationJobObject failed: error {err}"));
        }

        Ok(Self {
            handle: guard,
            name: job_name,
        })
    }

    pub(crate) fn assign_process(&self, process_handle: HANDLE) -> PocResult<()> {
        let ok = unsafe { AssignProcessToJobObject(self.handle.0, process_handle) };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            return Err(format!("AssignProcessToJobObject failed: error {err}"));
        }
        Ok(())
    }
}

/// Abstract representation of file metadata for path validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PocFileMetadata {
    pub is_symlink: bool,
    pub is_reparse_point: bool,
}

/// Pure ordered-component path inspection evaluator.
///
/// Evaluates path components progressively from the root component down to the leaf.
/// Enforces §151.2.6 requirements:
/// 1. Permitted path state consists of existing, non-reparse ancestors followed by an
///    optional contiguous absent suffix.
/// 2. Any present component discovered after an absent ancestor (`NotFound`) is strictly
///    rejected as a path integrity violation.
/// 3. Any non-NotFound I/O error fails closed immediately.
/// 4. Any component detected as a symlink or reparse point is rejected.
pub(crate) fn evaluate_path_components_integrity<F>(
    path: &Path,
    mut meta_lookup: F,
) -> Result<(), String>
where
    F: FnMut(&Path) -> std::io::Result<PocFileMetadata>,
{
    if !path.is_absolute() {
        return Err(format!("path '{}' must be absolute", path.display()));
    }

    let mut ancestors: Vec<&Path> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    ancestors.reverse(); // Ordered from root down to leaf: [root, prefix1, prefix2, ..., leaf]

    let mut saw_not_found = false;
    let mut absent_ancestor: Option<PathBuf> = None;

    for component in ancestors {
        match meta_lookup(component) {
            Ok(meta) => {
                if saw_not_found {
                    let absent_desc = absent_ancestor
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "earlier ancestor".to_string());
                    return Err(format!(
                        "path integrity violation: present component '{}' cannot exist after absent ancestor '{}'",
                        component.display(),
                        absent_desc
                    ));
                }
                if meta.is_symlink {
                    return Err(format!(
                        "path component '{}' must not be a symlink",
                        component.display()
                    ));
                }
                if meta.is_reparse_point {
                    return Err(format!(
                        "path component '{}' must not be a reparse point",
                        component.display()
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if !saw_not_found {
                    saw_not_found = true;
                    absent_ancestor = Some(component.to_path_buf());
                }
            }
            Err(e) => {
                return Err(format!(
                    "metadata inspection failed on path component '{}': {e}",
                    component.display()
                ));
            }
        }
    }

    Ok(())
}

/// Injected metadata lookup seam for testing path validation fail-closed behavior.
pub(crate) fn validate_poc_path_integrity_with_meta<F>(
    path: &Path,
    expected_root: &Path,
    mut meta_lookup: F,
) -> PocResult<PathBuf>
where
    F: FnMut(&Path) -> std::io::Result<PocFileMetadata>,
{
    if !path.is_absolute() {
        return Err(format!("path '{}' must be absolute", path.display()));
    }
    if !expected_root.is_absolute() {
        return Err(format!(
            "expected_root '{}' must be absolute",
            expected_root.display()
        ));
    }

    evaluate_path_components_integrity(path, &mut meta_lookup)?;

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize '{}': {e}", path.display()))?;
    let canonical_root = expected_root.canonicalize().map_err(|e| {
        format!(
            "cannot canonicalize expected_root '{}': {e}",
            expected_root.display()
        )
    })?;

    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "path '{}' (canonical: '{}') escapes expected root '{}' (canonical: '{}')",
            path.display(),
            canonical.display(),
            expected_root.display(),
            canonical_root.display()
        ));
    }

    for ancestor in canonical.ancestors() {
        if ancestor.starts_with(&canonical_root) {
            let meta = meta_lookup(ancestor).map_err(|e| {
                format!(
                    "metadata inspection failed on canonical component '{}': {e}",
                    ancestor.display()
                )
            })?;
            if meta.is_symlink {
                return Err(format!(
                    "canonical path component '{}' must not be a symlink",
                    ancestor.display()
                ));
            }
            if meta.is_reparse_point {
                return Err(format!(
                    "canonical path component '{}' must not be a reparse point",
                    ancestor.display()
                ));
            }
        }
    }

    Ok(canonical)
}

/// Validates path integrity using the actual filesystem metadata.
pub(crate) fn validate_poc_path_integrity(path: &Path, expected_root: &Path) -> PocResult<PathBuf> {
    validate_poc_path_integrity_with_meta(path, expected_root, |p| {
        let meta = std::fs::symlink_metadata(p)?;
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let is_reparse = meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
        Ok(PocFileMetadata {
            is_symlink: meta.file_type().is_symlink(),
            is_reparse_point: is_reparse,
        })
    })
}

/// Security descriptor snapshot for a path under test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PocSecurityDescriptorSnapshot {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

impl PocSecurityDescriptorSnapshot {
    pub(crate) fn capture(path: &Path) -> PocResult<Self> {
        let path_w = to_wide_null(path);
        let mut dacl: *mut ACL = null_mut();
        let mut sec_desc: PSECURITY_DESCRIPTOR = null_mut();

        let ret = unsafe {
            GetNamedSecurityInfoW(
                path_w.as_ptr(),
                SE_FILE_OBJECT,
                windows_sys::Win32::Security::DACL_SECURITY_INFORMATION
                    | windows_sys::Win32::Security::OWNER_SECURITY_INFORMATION
                    | windows_sys::Win32::Security::GROUP_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut sec_desc,
            )
        };
        if ret != ERROR_SUCCESS || sec_desc.is_null() {
            return Err(format!(
                "GetNamedSecurityInfoW failed to capture snapshot for '{}': error {ret}",
                path.display()
            ));
        }
        let _guard = PocAclGuard(sec_desc as _);

        let len = unsafe { GetSecurityDescriptorLength(sec_desc) };
        if len == 0 {
            return Err(format!(
                "GetSecurityDescriptorLength returned 0 for '{}'",
                path.display()
            ));
        }

        let slice = unsafe { std::slice::from_raw_parts(sec_desc as *const u8, len as usize) };
        Ok(Self {
            path: path.to_path_buf(),
            bytes: slice.to_vec(),
        })
    }
}

/// Observed filesystem metadata for canary verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PocObservedMetadata {
    pub file_length: u64,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub file_attributes: u32,
    pub content_hash: Option<String>,
}

/// Metadata baseline recorded for a protected canary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PocCanaryBaseline {
    pub file_name: String,
    pub content_hash: String,
    pub file_length: u64,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub file_attributes: u32,
}

impl PocCanaryBaseline {
    pub(crate) fn capture(path: &Path, expected_hash: String) -> PocResult<Self> {
        use std::os::windows::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| format!("cannot read metadata for '{}': {e}", path.display()))?;
        Ok(Self {
            file_name: path.file_name().unwrap().to_string_lossy().to_string(),
            content_hash: expected_hash,
            file_length: meta.len(),
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            is_symlink: meta.file_type().is_symlink(),
            file_attributes: meta.file_attributes(),
        })
    }

    /// Pure comparator testing observed metadata and content against baseline.
    /// Fails closed if any attribute, file type, length, or content hash deviates.
    pub(crate) fn compare_observed(&self, obs: &PocObservedMetadata) -> Result<(), String> {
        if obs.file_length != self.file_length {
            return Err(format!(
                "canary '{}' length modified: expected {}, got {}",
                self.file_name, self.file_length, obs.file_length
            ));
        }
        if obs.is_file != self.is_file
            || obs.is_dir != self.is_dir
            || obs.is_symlink != self.is_symlink
        {
            return Err(format!("canary '{}' file type modified", self.file_name));
        }
        if obs.file_attributes != self.file_attributes {
            return Err(format!(
                "canary '{}' file attributes modified: expected 0x{:X}, got 0x{:X}",
                self.file_name, self.file_attributes, obs.file_attributes
            ));
        }

        if self.is_file {
            match &obs.content_hash {
                Some(hash) if hash == &self.content_hash => {}
                Some(_) => {
                    return Err(format!(
                        "canary '{}' content modified (hash mismatch)",
                        self.file_name
                    ));
                }
                None => {
                    return Err(format!("canary '{}' missing content hash", self.file_name));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn verify_unmodified(&self, path: &Path) -> PocResult<()> {
        use std::os::windows::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| format!("cannot read metadata for canary '{}': {e}", path.display()))?;

        let content_hash = if self.is_file {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("cannot read canary bytes for '{}': {e}", path.display()))?;
            Some(sha256_digest(&bytes))
        } else {
            None
        };

        let obs = PocObservedMetadata {
            file_length: meta.len(),
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            is_symlink: meta.file_type().is_symlink(),
            file_attributes: meta.file_attributes(),
            content_hash,
        };

        self.compare_observed(&obs)
    }
}

/// Disposable synthetic fixture trees under a single owned `TempDir`.
pub(crate) struct PocFixture {
    pub tempdir: Option<tempfile::TempDir>,
    pub root_path: PathBuf,
    pub candidate_dir: PathBuf,
    pub scratch_dir: PathBuf,
    pub primary_dir: PathBuf,
    pub engine_state_dir: PathBuf,
    pub sentinels_dir: PathBuf,
    pub outside_dir: PathBuf,
    pub canaries: Vec<PocCanaryBaseline>,
    pub candidate_initial_snapshot: PocSecurityDescriptorSnapshot,
    pub scratch_initial_snapshot: PocSecurityDescriptorSnapshot,
    pub mutated_paths: Vec<PathBuf>,
    pub recovery_required: bool,
}

/// Detailed record of a single target restoration attempt and verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetRestorationRecord {
    pub target: PathBuf,
    pub restored: bool,
    pub verified: bool,
    pub error: Option<String>,
}

/// Structured aggregate report of multi-target restoration attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MultiTargetRestorationReport {
    pub records: Vec<TargetRestorationRecord>,
}

impl MultiTargetRestorationReport {
    pub(crate) fn is_all_success(&self) -> bool {
        !self.records.is_empty()
            && self
                .records
                .iter()
                .all(|r| r.restored && r.verified && r.error.is_none())
    }

    pub(crate) fn combined_error(&self) -> Option<String> {
        let errs: Vec<_> = self
            .records
            .iter()
            .filter_map(|r| {
                r.error
                    .as_ref()
                    .map(|e| format!("'{}': {e}", r.target.display()))
            })
            .collect();
        if errs.is_empty() {
            None
        } else {
            Some(errs.join("; "))
        }
    }
}

/// Attempts every target even if an earlier restoration fails or panics,
/// recording per-target attempt, result, and verification status.
pub(crate) fn restore_all_targets_recorded<F>(
    targets: &[PathBuf],
    mut restore_and_verify_one: F,
) -> MultiTargetRestorationReport
where
    F: FnMut(&Path) -> Result<(), String>,
{
    let mut records = Vec::with_capacity(targets.len());
    for target in targets {
        let res = catch_unwind(AssertUnwindSafe(|| {
            restore_and_verify_one(target.as_path())
        }));
        match res {
            Ok(Ok(())) => {
                records.push(TargetRestorationRecord {
                    target: target.clone(),
                    restored: true,
                    verified: true,
                    error: None,
                });
            }
            Ok(Err(err)) => {
                records.push(TargetRestorationRecord {
                    target: target.clone(),
                    restored: false,
                    verified: false,
                    error: Some(err),
                });
            }
            Err(payload) => {
                records.push(TargetRestorationRecord {
                    target: target.clone(),
                    restored: false,
                    verified: false,
                    error: Some(format!(
                        "restoration panicked: {}",
                        panic_payload_message(payload)
                    )),
                });
            }
        }
    }
    MultiTargetRestorationReport { records }
}

/// Attempts every target even if an earlier restoration fails or panics.
fn restore_all_targets<F>(targets: &[PathBuf], restore_one: F) -> Result<(), String>
where
    F: FnMut(&Path) -> Result<(), String>,
{
    let report = restore_all_targets_recorded(targets, restore_one);
    if let Some(err) = report.combined_error() {
        Err(err)
    } else {
        Ok(())
    }
}

impl PocFixture {
    pub(crate) fn new() -> PocResult<Self> {
        let tempdir = tempfile::tempdir().map_err(|e| format!("failed to create TempDir: {e}"))?;
        let root = tempdir.path().to_path_buf();

        let candidate_dir = root.join("candidate");
        let scratch_dir = root.join("scratch");
        let primary_dir = root.join("primary");
        let engine_state_dir = root.join("engine_state");
        let sentinels_dir = root.join("sentinels");
        let outside_dir = root.join("outside");

        for d in [
            &candidate_dir,
            &scratch_dir,
            &primary_dir,
            &engine_state_dir,
            &sentinels_dir,
            &outside_dir,
        ] {
            std::fs::create_dir_all(d)
                .map_err(|e| format!("failed to create fixture dir '{}': {e}", d.display()))?;
        }

        let mut canaries = Vec::new();

        let primary_baseline = primary_dir.join("protected_source.rs");
        let primary_append = primary_dir.join("append_canary.rs");
        let primary_overwrite = primary_dir.join("overwrite_canary.rs");
        let primary_delete = primary_dir.join("delete_canary.rs");
        let primary_rename = primary_dir.join("rename_canary.rs");
        let primary_subdir = primary_dir.join("existing_subdir");
        std::fs::create_dir_all(&primary_subdir)
            .map_err(|e| format!("mkdir primary_subdir: {e}"))?;

        let engine_baseline = engine_state_dir.join("protected_state.json");
        let engine_overwrite = engine_state_dir.join("engine_overwrite_canary.json");

        let sentinels_canary = sentinels_dir.join("untracked_scratch.txt");
        let outside_canary = outside_dir.join("outside_canary.txt");

        let files: &[(&Path, &[u8])] = &[
            (
                &primary_baseline,
                b"pub fn protected() { /* immutable baseline */ }\n",
            ),
            (
                &primary_append,
                b"pub fn append_canary() { /* append target */ }\n",
            ),
            (
                &primary_overwrite,
                b"pub fn overwrite_canary() { /* overwrite target */ }\n",
            ),
            (
                &primary_delete,
                b"pub fn delete_canary() { /* delete target */ }\n",
            ),
            (
                &primary_rename,
                b"pub fn rename_canary() { /* rename target */ }\n",
            ),
            (
                &engine_baseline,
                b"{\"state\": \"locked\", \"version\": 1}\n",
            ),
            (&engine_overwrite, b"{\"state\": \"overwrite_target\"}\n"),
            (&sentinels_canary, b"untracked user work sentinel canary\n"),
            (&outside_canary, b"canary outside workspace boundary\n"),
        ];

        for (path, content) in files {
            std::fs::write(path, content)
                .map_err(|e| format!("write canary '{}': {e}", path.display()))?;
            let hash = sha256_digest(content);
            canaries.push(PocCanaryBaseline::capture(path, hash)?);
        }
        canaries.push(PocCanaryBaseline::capture(&primary_subdir, String::new())?);

        // Mandatory capture of security descriptors before mutation
        let candidate_snap = PocSecurityDescriptorSnapshot::capture(&candidate_dir)?;
        let scratch_snap = PocSecurityDescriptorSnapshot::capture(&scratch_dir)?;

        Ok(Self {
            tempdir: Some(tempdir),
            root_path: root,
            candidate_dir,
            scratch_dir,
            primary_dir,
            engine_state_dir,
            sentinels_dir,
            outside_dir,
            canaries,
            candidate_initial_snapshot: candidate_snap,
            scratch_initial_snapshot: scratch_snap,
            mutated_paths: Vec::new(),
            recovery_required: false,
        })
    }

    /// Verifies all protected canaries have content hashes and metadata matching baseline.
    pub(crate) fn verify_canaries_unmodified(&self) -> PocResult<()> {
        for canary in &self.canaries {
            let file_path = if canary.file_name.contains("engine") {
                self.engine_state_dir.join(&canary.file_name)
            } else if canary.file_name.contains("untracked") {
                self.sentinels_dir.join(&canary.file_name)
            } else if canary.file_name.contains("outside") {
                self.outside_dir.join(&canary.file_name)
            } else {
                self.primary_dir.join(&canary.file_name)
            };
            canary.verify_unmodified(&file_path)?;
        }
        Ok(())
    }

    /// Grants write ACEs for the restriction SID ONLY on candidate and scratch roots.
    pub(crate) fn apply_restriction_sid_write_grants(
        &mut self,
        restriction_sid: &PocRestrictionSid,
    ) -> PocResult<()> {
        let root = self.root_path.clone();
        for target in [&self.candidate_dir, &self.scratch_dir] {
            validate_poc_path_integrity(target, &root)?;

            let target_w = to_wide_null(target);
            let mut old_dacl: *mut ACL = null_mut();
            let mut sec_desc: PSECURITY_DESCRIPTOR = null_mut();

            let get_ret = unsafe {
                GetNamedSecurityInfoW(
                    target_w.as_ptr(),
                    SE_FILE_OBJECT,
                    windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    &mut old_dacl,
                    null_mut(),
                    &mut sec_desc,
                )
            };
            if get_ret != ERROR_SUCCESS {
                self.recovery_required = true;
                return Err(format!(
                    "GetNamedSecurityInfoW failed on '{}': error {get_ret}",
                    target.display()
                ));
            }
            let _sec_desc_guard = PocAclGuard(sec_desc as _);

            const GRANTED_ACCESS: u32 = FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE;
            let mut ea: EXPLICIT_ACCESS_W = unsafe { std::mem::zeroed() };
            ea.grfAccessPermissions = GRANTED_ACCESS;
            ea.grfAccessMode = GRANT_ACCESS;
            ea.grfInheritance = windows_sys::Win32::Security::CONTAINER_INHERIT_ACE
                | windows_sys::Win32::Security::OBJECT_INHERIT_ACE;
            ea.Trustee.pMultipleTrustee = null_mut();
            ea.Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;
            ea.Trustee.TrusteeForm = TRUSTEE_IS_SID;
            ea.Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
            ea.Trustee.ptstrName = restriction_sid.sid.0 as _;

            let mut new_dacl: *mut ACL = null_mut();
            let set_ret = unsafe { SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
            if set_ret != ERROR_SUCCESS {
                self.recovery_required = true;
                return Err(format!(
                    "SetEntriesInAclW failed for '{}': error {set_ret}",
                    target.display()
                ));
            }
            let _new_dacl_guard = PocAclGuard(new_dacl);

            // Register the target before the mutating API call so unwind or an ambiguous
            // API failure cannot leave an untracked partially-mutated descriptor.
            if !self.mutated_paths.contains(target) {
                self.mutated_paths.push((*target).clone());
            }

            let apply_ret = unsafe {
                SetNamedSecurityInfoW(
                    target_w.as_ptr() as _,
                    SE_FILE_OBJECT,
                    windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    new_dacl,
                    null_mut(),
                )
            };
            if apply_ret != ERROR_SUCCESS {
                self.recovery_required = true;
                return Err(format!(
                    "SetNamedSecurityInfoW failed for '{}': error {apply_ret}",
                    target.display()
                ));
            }
        }

        Ok(())
    }

    /// Restores security descriptors on all mutated paths and compares against baseline.
    /// Collects per-target attempt, outcome, and verification in a structured report.
    pub(crate) fn restore_security_descriptors_detailed(&mut self) -> MultiTargetRestorationReport {
        let targets = self.mutated_paths.clone();
        let candidate_snap = self.candidate_initial_snapshot.clone();
        let scratch_snap = self.scratch_initial_snapshot.clone();
        let candidate_dir = self.candidate_dir.clone();
        let scratch_dir = self.scratch_dir.clone();

        let report = restore_all_targets_recorded(&targets, |target| {
            let snap = if target == &candidate_dir {
                &candidate_snap
            } else if target == &scratch_dir {
                &scratch_snap
            } else {
                return Err("unknown mutated target".into());
            };

            let sec_desc = snap.bytes.as_ptr() as PSECURITY_DESCRIPTOR;
            let mut dacl: *mut ACL = null_mut();
            let mut dacl_present: i32 = 0;
            let mut dacl_defaulted: i32 = 0;

            let ok = unsafe {
                windows_sys::Win32::Security::GetSecurityDescriptorDacl(
                    sec_desc,
                    &mut dacl_present,
                    &mut dacl,
                    &mut dacl_defaulted,
                )
            };
            if ok == 0 {
                return Err("GetSecurityDescriptorDacl failed".into());
            }

            let target_w = to_wide_null(target);
            let set_ret = unsafe {
                SetNamedSecurityInfoW(
                    target_w.as_ptr() as _,
                    SE_FILE_OBJECT,
                    windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    dacl,
                    null_mut(),
                )
            };
            if set_ret != ERROR_SUCCESS {
                return Err(format!("restoring DACL failed: error {set_ret}"));
            }

            let current = match PocSecurityDescriptorSnapshot::capture(target) {
                Ok(current) => current,
                Err(err) => {
                    return Err(format!("verifying restored descriptor: {err}"));
                }
            };
            if current.bytes != snap.bytes {
                return Err("restored descriptor does not match initial snapshot".into());
            }
            Ok(())
        });
        if !report.is_all_success() {
            self.recovery_required = true;
        }
        report
    }

    /// Restores security descriptors on all mutated paths and compares against baseline.
    pub(crate) fn restore_security_descriptors(&mut self) -> PocResult<()> {
        let report = self.restore_security_descriptors_detailed();
        if report.records.is_empty() {
            return Ok(());
        }
        if let Some(err) = report.combined_error() {
            self.recovery_required = true;
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Preserves the fixture directory when recovery is required, preventing drop cleanup.
    pub(crate) fn preserve_on_recovery(&mut self) -> PathBuf {
        let path = self.root_path.clone();
        if let Some(td) = self.tempdir.take() {
            #[allow(deprecated)]
            let _ = td.into_path();
        }
        path
    }

    /// Centralized post-mutation exit handler for the fixture delegating directly to the unified decision seam.
    pub(crate) fn handle_post_mutation_exit(
        &mut self,
        run_err: Option<String>,
    ) -> PocRecoveryOutcome {
        let outcome = handle_post_mutation_decision(self, run_err);
        if outcome.fixture_preserved || !outcome.restore_succeeded {
            self.recovery_required = true;
        }
        outcome
    }
}

/// Abstract recovery target enabling unified, testable post-mutation decision handling.
pub(crate) trait PocRecoveryTarget {
    fn restore_target(&mut self) -> Result<(), String>;
    fn preserve_target(&mut self) -> PathBuf;
    fn cleanup_target(&mut self);
}

impl PocRecoveryTarget for PocFixture {
    fn restore_target(&mut self) -> Result<(), String> {
        self.restore_security_descriptors()
    }

    fn preserve_target(&mut self) -> PathBuf {
        self.preserve_on_recovery()
    }

    fn cleanup_target(&mut self) {
        self.mutated_paths.clear();
        self.recovery_required = false;
    }
}

impl Drop for PocFixture {
    fn drop(&mut self) {
        // Enforces §153.2 requirement 1:
        // If a panic or unhandled failure occurs while mutated paths exist or recovery is required,
        // prevent TempDir drop from destroying the investigation fixture.
        if self.recovery_required || !self.mutated_paths.is_empty() {
            let _ = self.preserve_on_recovery();
        }
    }
}

/// Post-mutation state of the test fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PocPostMutationState {
    RestoreAttemptedSuccess,
    RestoreAttemptedFailed,
    FixtureCleanedUp,
}

/// Structured outcome of post-mutation exit handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PocRecoveryOutcome {
    pub restore_attempted: bool,
    pub restore_succeeded: bool,
    pub fixture_preserved: bool,
    pub final_state: PocPostMutationState,
    pub error_message: Option<String>,
}

fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        "non-string panic payload".to_string()
    }
}

fn run_post_mutation_catching_panic<F>(run: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|payload| {
        Err(format!(
            "post-mutation execution panicked: {}",
            panic_payload_message(payload)
        ))
    })
}

/// Centralized post-mutation exit decision state machine.
/// Enforces §151.2.5 and §153.2.1 requirements:
/// 1. On EVERY failure (whether in child execution, assertions, canary checks, or unwind),
///    the defined security descriptor restoration is attempted EXACTLY ONCE.
/// 2. If the run failed, or if restoration failed/panicked, the target is preserved on disk.
/// 3. Normal cleanup is permitted ONLY when the run succeeded AND restoration succeeded.
pub(crate) fn handle_post_mutation_decision<T: PocRecoveryTarget + ?Sized>(
    target: &mut T,
    run_err: Option<String>,
) -> PocRecoveryOutcome {
    let mut restore_safely = || match catch_unwind(AssertUnwindSafe(|| target.restore_target())) {
        Ok(result) => result,
        Err(payload) => Err(format!(
            "restoration panicked: {}",
            panic_payload_message(payload)
        )),
    };
    if let Some(err) = run_err {
        let restore_res = restore_safely();
        let restore_succeeded = restore_res.is_ok();
        // Since the run itself encountered an error, uncertainty is present; preserve target.
        let _preserved_path = target.preserve_target();
        PocRecoveryOutcome {
            restore_attempted: true,
            restore_succeeded,
            fixture_preserved: true,
            final_state: if restore_succeeded {
                PocPostMutationState::RestoreAttemptedSuccess
            } else {
                PocPostMutationState::RestoreAttemptedFailed
            },
            error_message: Some(err),
        }
    } else {
        match restore_safely() {
            Ok(()) => {
                target.cleanup_target();
                PocRecoveryOutcome {
                    restore_attempted: true,
                    restore_succeeded: true,
                    fixture_preserved: false,
                    final_state: PocPostMutationState::FixtureCleanedUp,
                    error_message: None,
                }
            }
            Err(restore_err) => {
                let _preserved_path = target.preserve_target();
                PocRecoveryOutcome {
                    restore_attempted: true,
                    restore_succeeded: false,
                    fixture_preserved: true,
                    final_state: PocPostMutationState::RestoreAttemptedFailed,
                    error_message: Some(format!("restoration failed: {restore_err}")),
                }
            }
        }
    }
}

struct ClosureRecoveryTarget<R, P, C> {
    restore_fn: R,
    preserve_fn: P,
    cleanup_fn: C,
}

impl<R, P, C> PocRecoveryTarget for ClosureRecoveryTarget<R, P, C>
where
    R: FnMut() -> Result<(), String>,
    P: FnMut() -> PathBuf,
    C: FnMut(),
{
    fn restore_target(&mut self) -> Result<(), String> {
        (self.restore_fn)()
    }

    fn preserve_target(&mut self) -> PathBuf {
        (self.preserve_fn)()
    }

    fn cleanup_target(&mut self) {
        (self.cleanup_fn)()
    }
}

/// Centralized post-mutation exit handler delegating directly to the unified decision seam.
pub(crate) fn handle_post_mutation_exit<R, P, C>(
    run_err: Option<String>,
    restore_fn: R,
    preserve_fn: P,
    cleanup_fn: C,
) -> PocRecoveryOutcome
where
    R: FnMut() -> Result<(), String>,
    P: FnMut() -> PathBuf,
    C: FnMut(),
{
    let mut target = ClosureRecoveryTarget {
        restore_fn,
        preserve_fn,
        cleanup_fn,
    };
    handle_post_mutation_decision(&mut target, run_err)
}

/// Documented policy bounds for mandatory integrity RID in restricted token.
pub(crate) const SECURITY_MANDATORY_LOW_RID: u32 = 0x1000;
pub(crate) const SECURITY_MANDATORY_MEDIUM_RID: u32 = 0x2000;

/// Accept only the canonical decimal spelling emitted by Windows SID conversion.
fn is_canonical_sid(value: &str) -> bool {
    let mut parts = value.split('-');
    if parts.next() != Some("S") || parts.next() != Some("1") {
        return false;
    }
    let Some(authority) = parts.next().and_then(|p| p.parse::<u64>().ok()) else {
        return false;
    };
    if authority > 0x0000_FFFF_FFFF_FFFF {
        return false;
    }
    let subs: Vec<_> = parts.collect();
    subs.len() <= 15
        && subs.iter().all(|part| {
            part.parse::<u32>()
                .map(|n| n.to_string() == *part)
                .unwrap_or(false)
        })
        && authority.to_string() == value.split('-').nth(2).unwrap_or_default()
}

fn sid_bytes_to_canonical(bytes: &[u8]) -> Result<(String, u64, Vec<u32>), String> {
    if bytes.len() < 8 || bytes[0] != 1 || bytes[1] > 15 {
        return Err("malformed SID header".into());
    }
    let count = bytes[1] as usize;
    let required = 8usize
        .checked_add(count.checked_mul(4).ok_or("SID length overflow")?)
        .ok_or("SID length overflow")?;
    if bytes.len() < required {
        return Err("truncated SID subauthorities".into());
    }
    if bytes.len() != required {
        return Err("SID buffer contains trailing bytes".into());
    }
    let mut authority = 0u64;
    for byte in &bytes[2..8] {
        authority = (authority << 8) | u64::from(*byte);
    }
    let mut subs = Vec::with_capacity(count);
    for chunk in bytes[8..required].chunks_exact(4) {
        subs.push(u32::from_le_bytes(
            chunk.try_into().expect("four-byte chunk"),
        ));
    }
    let text = std::iter::once("S".to_string())
        .chain(std::iter::once("1".to_string()))
        .chain(std::iter::once(authority.to_string()))
        .chain(subs.iter().map(u32::to_string))
        .collect::<Vec<_>>()
        .join("-");
    Ok((text, authority, subs))
}

fn checked_token_region(bytes: &[u8], start: usize, length: usize) -> Result<&[u8], String> {
    let end = start
        .checked_add(length)
        .ok_or("token buffer range overflow")?;
    bytes
        .get(start..end)
        .ok_or_else(|| "truncated token information buffer".into())
}

fn checked_sid_pointer_region<'a>(
    bytes: &'a [u8],
    base_address: usize,
    sid_address: usize,
) -> Result<&'a [u8], String> {
    let offset = sid_address
        .checked_sub(base_address)
        .ok_or("SID pointer precedes token buffer")?;
    let header = checked_token_region(bytes, offset, 8)?;
    let sid_len = 8usize
        .checked_add(
            usize::from(header[1])
                .checked_mul(4)
                .ok_or("SID length overflow")?,
        )
        .ok_or("SID length overflow")?;
    checked_token_region(bytes, offset, sid_len)
}

fn checked_array_len(
    header_size: usize,
    count: usize,
    entry_size: usize,
    len: usize,
) -> Result<(), String> {
    let entries = count
        .checked_mul(entry_size)
        .ok_or("token entry count overflow")?;
    let required = header_size
        .checked_add(entries)
        .ok_or("token buffer size overflow")?;
    if required > len {
        return Err("token information count exceeds returned buffer length".into());
    }
    Ok(())
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let raw: [u8; 4] = checked_token_region(bytes, offset, 4)?
        .try_into()
        .map_err(|_| "invalid u32 field")?;
    Ok(u32::from_ne_bytes(raw))
}

fn read_pointer_at(bytes: &[u8], offset: usize) -> Result<usize, String> {
    let raw = checked_token_region(bytes, offset, std::mem::size_of::<usize>())?;
    let mut value = 0usize;
    for (index, byte) in raw.iter().enumerate() {
        value |= usize::from(*byte) << (index * 8);
    }
    Ok(value)
}

fn sid_string_from_bytes(
    bytes: &[u8],
    base_address: usize,
    sid_addr: usize,
    minimum_sid_offset: usize,
) -> Result<(String, u64, Vec<u32>), String> {
    let offset = sid_addr
        .checked_sub(base_address)
        .ok_or("token SID pointer precedes returned buffer")?;
    if offset < minimum_sid_offset {
        return Err("token SID pointer overlaps fixed structure header".into());
    }
    let sid_bytes = checked_sid_pointer_region(bytes, base_address, sid_addr)?;
    sid_bytes_to_canonical(sid_bytes)
}

pub(crate) fn parse_token_user_from_bytes(
    bytes: &[u8],
    base_address: usize,
) -> Result<String, String> {
    let header_size = std::mem::size_of::<TOKEN_USER>();
    if bytes.len() < header_size {
        return Err("invalid TokenUser returned length".into());
    }
    let sid_offset =
        std::mem::offset_of!(TOKEN_USER, User) + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid);
    let sid_addr = read_pointer_at(bytes, sid_offset)?;
    let (sid, _, _) = sid_string_from_bytes(bytes, base_address, sid_addr, header_size)?;
    Ok(sid)
}

pub(crate) fn parse_token_groups_from_bytes(
    bytes: &[u8],
    base_address: usize,
) -> Result<Vec<String>, String> {
    if bytes.len() < std::mem::size_of::<u32>() {
        return Err("invalid TokenGroups returned length".into());
    }
    let count = read_u32_at(bytes, 0)? as usize;
    let entries_offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    let entry_size = std::mem::size_of::<SID_AND_ATTRIBUTES>();
    checked_array_len(entries_offset, count, entry_size, bytes.len())?;
    if count == 0 {
        return Err("TokenGroups contains no groups".into());
    }
    let sid_field_offset = std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid);
    let minimum_sid_offset = entries_offset
        .checked_add(
            count
                .checked_mul(entry_size)
                .ok_or("group count overflow")?,
        )
        .ok_or("group header size overflow")?;
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let entry = entries_offset + index * entry_size;
        let sid_addr = read_pointer_at(bytes, entry + sid_field_offset)?;
        let (sid, _, _) = sid_string_from_bytes(bytes, base_address, sid_addr, minimum_sid_offset)?;
        result.push(sid);
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PocPrivilegeLuid {
    pub low_part: u32,
    pub high_part: i32,
}

pub(crate) const SE_CHANGE_NOTIFY_PRIVILEGE_LUID: PocPrivilegeLuid = PocPrivilegeLuid {
    low_part: 23,
    high_part: 0,
};

pub(crate) fn parse_token_privileges_from_bytes(
    bytes: &[u8],
) -> Result<(u32, Vec<PocPrivilegeLuid>), String> {
    if bytes.len() < std::mem::size_of::<u32>() {
        return Err("invalid TokenPrivileges returned length".into());
    }
    let count = read_u32_at(bytes, 0)? as usize;
    let entries_offset = std::mem::offset_of!(TOKEN_PRIVILEGES, Privileges);
    let entry_size = std::mem::size_of::<windows_sys::Win32::Security::LUID_AND_ATTRIBUTES>();
    checked_array_len(entries_offset, count, entry_size, bytes.len())?;
    if count == 0 || count > u32::MAX as usize {
        return Err("TokenPrivileges count is empty or invalid".into());
    }
    let luid_offset = std::mem::offset_of!(windows_sys::Win32::Security::LUID_AND_ATTRIBUTES, Luid);
    let low_part_offset = std::mem::offset_of!(windows_sys::Win32::Foundation::LUID, LowPart);
    let high_part_offset = std::mem::offset_of!(windows_sys::Win32::Foundation::LUID, HighPart);
    let attrs_offset = std::mem::offset_of!(
        windows_sys::Win32::Security::LUID_AND_ATTRIBUTES,
        Attributes
    );
    const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
    let mut enabled = Vec::new();
    for index in 0..count {
        let entry_offset = entries_offset + index * entry_size;
        let attrs = read_u32_at(bytes, entry_offset + attrs_offset)?;
        if attrs & SE_PRIVILEGE_ENABLED != 0 {
            enabled.push(PocPrivilegeLuid {
                low_part: read_u32_at(bytes, entry_offset + luid_offset + low_part_offset)?,
                high_part: read_u32_at(bytes, entry_offset + luid_offset + high_part_offset)?
                    as i32,
            });
        }
    }
    Ok((count as u32, enabled))
}

pub(crate) fn parse_token_integrity_from_bytes(
    bytes: &[u8],
    base_address: usize,
) -> Result<(String, u32), String> {
    let header_size = std::mem::size_of::<TOKEN_MANDATORY_LABEL>();
    if bytes.len() < header_size {
        return Err("invalid TokenIntegrityLevel returned length".into());
    }
    let sid_offset = std::mem::offset_of!(TOKEN_MANDATORY_LABEL, Label)
        + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid);
    let sid_addr = read_pointer_at(bytes, sid_offset)?;
    let (sid, authority, subauthorities) =
        sid_string_from_bytes(bytes, base_address, sid_addr, header_size)?;
    if authority != 16 || subauthorities.len() != 1 {
        return Err("TokenIntegrityLevel SID is not a canonical mandatory-label SID".into());
    }
    Ok((sid, subauthorities[0]))
}

unsafe fn validate_token_user_buffer(
    buffer: *const u8,
    returned_len: u32,
    capacity: usize,
) -> Result<String, String> {
    let len = returned_len as usize;
    if buffer.is_null() || len == 0 || len > capacity {
        return Err("invalid TokenUser returned length".into());
    }
    let bytes = std::slice::from_raw_parts(buffer, len);
    parse_token_user_from_bytes(bytes, buffer as usize)
}

unsafe fn validate_token_groups_buffer(
    buffer: *const u8,
    returned_len: u32,
    capacity: usize,
) -> Result<Vec<String>, String> {
    let len = returned_len as usize;
    if buffer.is_null() || len == 0 || len > capacity {
        return Err("invalid TokenGroups returned length".into());
    }
    let bytes = std::slice::from_raw_parts(buffer, len);
    parse_token_groups_from_bytes(bytes, buffer as usize)
}

unsafe fn validate_token_privileges_buffer(
    buffer: *const u8,
    returned_len: u32,
    capacity: usize,
) -> Result<(u32, Vec<PocPrivilegeLuid>), String> {
    let len = returned_len as usize;
    if buffer.is_null() || len == 0 || len > capacity {
        return Err("invalid TokenPrivileges returned length".into());
    }
    let bytes = std::slice::from_raw_parts(buffer, len);
    parse_token_privileges_from_bytes(bytes)
}

unsafe fn validate_token_integrity_buffer(
    buffer: *const u8,
    returned_len: u32,
    capacity: usize,
) -> Result<(String, u32), String> {
    let len = returned_len as usize;
    if buffer.is_null() || len == 0 || len > capacity {
        return Err("invalid TokenIntegrityLevel returned length".into());
    }
    let bytes = std::slice::from_raw_parts(buffer, len);
    parse_token_integrity_from_bytes(bytes, buffer as usize)
}

/// Token evidence queried and observed from the created restricted token.
/// Contains only observed evidence (no request constants asserted as proof).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PocTokenEvidence {
    pub observed_user_sid: String,
    pub expected_user_sid: String,
    pub observed_restricted_sids: Vec<String>,
    pub expected_restriction_sid: String,
    pub privilege_count: u32,
    pub enabled_privilege_count: u32,
    pub enabled_privilege_luids: Vec<PocPrivilegeLuid>,
    pub integrity_level_rid: u32,
    pub is_token_restricted_query: bool,
}

impl PocTokenEvidence {
    /// Validates observed token evidence against security policy requirements:
    /// - `TokenUser` matches the controller user SID and is non-empty canonical SID.
    /// - `TokenRestrictedSids` contains the expected per-run restriction SID.
    /// - `IsTokenRestricted` query returned true.
    /// - `TokenPrivileges` has no enabled privileges except the documented
    ///   `SeChangeNotifyPrivilege` exception for `DISABLE_MAX_PRIVILEGE`.
    /// - `TokenIntegrityLevel` RID falls within allowed policy `[0x1000, 0x2000]`.
    pub(crate) fn validate_evidence(&self) -> Result<(), String> {
        if !is_canonical_sid(&self.observed_user_sid) {
            return Err(format!(
                "token evidence: observed user SID is invalid or malformed: '{}'",
                self.observed_user_sid
            ));
        }
        if !is_canonical_sid(&self.expected_user_sid) {
            return Err(format!(
                "token evidence: expected user SID is invalid or malformed: '{}'",
                self.expected_user_sid
            ));
        }
        if self.observed_user_sid != self.expected_user_sid {
            return Err(format!(
                "token evidence: user SID mismatch: observed '{}', expected '{}'",
                self.observed_user_sid, self.expected_user_sid
            ));
        }

        if !is_canonical_sid(&self.expected_restriction_sid) {
            return Err(format!(
                "token evidence: expected restriction SID is invalid: '{}'",
                self.expected_restriction_sid
            ));
        }
        if self.observed_restricted_sids.is_empty() {
            return Err("token evidence: no restricted SIDs observed in token".into());
        }
        if self
            .observed_restricted_sids
            .iter()
            .any(|sid| !is_canonical_sid(sid))
        {
            return Err("token evidence: malformed restricted SID observed".into());
        }
        if !self
            .observed_restricted_sids
            .contains(&self.expected_restriction_sid)
        {
            return Err(format!(
                "token evidence: expected restriction SID '{}' absent from observed restricted SIDs {:?}",
                self.expected_restriction_sid, self.observed_restricted_sids
            ));
        }

        if !self.is_token_restricted_query {
            return Err(
                "token evidence: IsTokenRestricted returned false for restricted token".into(),
            );
        }

        if self.privilege_count == 0 {
            return Err(
                "token evidence: token privilege query returned 0 privileges (unqueried or malformed)"
                    .into(),
            );
        }
        if self.enabled_privilege_count as usize != self.enabled_privilege_luids.len() {
            return Err(format!(
                "token evidence: enabled privilege count {} does not match observed LUID evidence count {}",
                self.enabled_privilege_count,
                self.enabled_privilege_luids.len()
            ));
        }
        if self
            .enabled_privilege_luids
            .iter()
            .any(|luid| *luid != SE_CHANGE_NOTIFY_PRIVILEGE_LUID)
            || self.enabled_privilege_luids.len() > 1
        {
            return Err(format!(
                "token evidence: unexpected enabled privilege LUIDs under DISABLE_MAX_PRIVILEGE: {:?}",
                self.enabled_privilege_luids
            ));
        }

        if self.integrity_level_rid < SECURITY_MANDATORY_LOW_RID
            || self.integrity_level_rid > SECURITY_MANDATORY_MEDIUM_RID
        {
            return Err(format!(
                "token evidence: integrity RID 0x{:X} outside allowed restricted policy [0x1000, 0x2000]",
                self.integrity_level_rid
            ));
        }

        Ok(())
    }
}

/// Creates a restricted token from the caller's process token with
/// `WRITE_RESTRICTED | DISABLE_MAX_PRIVILEGE` and the specified restriction SID.
pub(crate) fn create_poc_restricted_token(
    restriction_sid: &PocRestrictionSid,
) -> PocResult<(PocHandleGuard, PocTokenEvidence)> {
    let mut current_token: HANDLE = null_mut();
    let ok = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
            &mut current_token,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("OpenProcessToken failed: error {err}"));
    }
    let current_token_guard = PocHandleGuard(current_token);

    // Get expected controller user SID
    let mut exp_user_len: u32 = 0;
    unsafe {
        GetTokenInformation(current_token, TokenUser, null_mut(), 0, &mut exp_user_len);
    }
    if exp_user_len == 0 {
        return Err("GetTokenInformation for controller TokenUser returned 0".into());
    }
    let mut exp_user_buf = AlignedBuffer::new(exp_user_len as usize);
    let ok = unsafe {
        GetTokenInformation(
            current_token,
            TokenUser,
            exp_user_buf.as_mut_ptr() as _,
            exp_user_buf.byte_capacity() as u32,
            &mut exp_user_len,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "GetTokenInformation for controller TokenUser failed: error {err}"
        ));
    }
    let expected_user_sid = unsafe {
        validate_token_user_buffer(
            exp_user_buf.as_ptr(),
            exp_user_len,
            exp_user_buf.byte_capacity(),
        )?
    };

    let sid_and_attr = SID_AND_ATTRIBUTES {
        Sid: restriction_sid.sid.0,
        Attributes: 0,
    };

    let flags = WRITE_RESTRICTED | DISABLE_MAX_PRIVILEGE;
    let mut restricted_token: HANDLE = null_mut();
    let ok = unsafe {
        CreateRestrictedToken(
            current_token_guard.0,
            flags,
            0,
            null(),
            0,
            null(),
            1,
            &sid_and_attr,
            &mut restricted_token,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("CreateRestrictedToken failed: error {err}"));
    }
    let restricted_guard = PocHandleGuard(restricted_token);

    let is_restricted = unsafe { IsTokenRestricted(restricted_token) != 0 };

    let mut evidence = PocTokenEvidence {
        expected_user_sid,
        expected_restriction_sid: restriction_sid.string_representation.clone(),
        is_token_restricted_query: is_restricted,
        ..Default::default()
    };

    // 1. Verify TokenUser matches expected controller user SID
    let mut user_len: u32 = 0;
    unsafe {
        GetTokenInformation(restricted_token, TokenUser, null_mut(), 0, &mut user_len);
    }
    if user_len == 0 {
        return Err("TokenUser query returned 0 length".into());
    }
    let mut user_buf = AlignedBuffer::new(user_len as usize);
    let ok = unsafe {
        GetTokenInformation(
            restricted_token,
            TokenUser,
            user_buf.as_mut_ptr() as _,
            user_buf.byte_capacity() as u32,
            &mut user_len,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("TokenUser query failed: error {err}"));
    }
    evidence.observed_user_sid = unsafe {
        validate_token_user_buffer(user_buf.as_ptr(), user_len, user_buf.byte_capacity())?
    };

    // 2. Verify TokenRestrictedSids
    let mut len: u32 = 0;
    unsafe {
        GetTokenInformation(
            restricted_token,
            TokenRestrictedSids,
            null_mut(),
            0,
            &mut len,
        );
    }
    if len == 0 {
        return Err("TokenRestrictedSids query returned 0 length".into());
    }
    let mut buf = AlignedBuffer::new(len as usize);
    let ok = unsafe {
        GetTokenInformation(
            restricted_token,
            TokenRestrictedSids,
            buf.as_mut_ptr() as _,
            buf.byte_capacity() as u32,
            &mut len,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("TokenRestrictedSids query failed: error {err}"));
    }

    evidence.observed_restricted_sids =
        unsafe { validate_token_groups_buffer(buf.as_ptr(), len, buf.byte_capacity())? };

    // 3. Verify TokenPrivileges are disabled or removed
    let mut priv_len: u32 = 0;
    unsafe {
        GetTokenInformation(
            restricted_token,
            TokenPrivileges,
            null_mut(),
            0,
            &mut priv_len,
        );
    }
    if priv_len == 0 {
        return Err("TokenPrivileges query returned 0 length".into());
    }
    let mut priv_buf = AlignedBuffer::new(priv_len as usize);
    let ok = unsafe {
        GetTokenInformation(
            restricted_token,
            TokenPrivileges,
            priv_buf.as_mut_ptr() as _,
            priv_buf.byte_capacity() as u32,
            &mut priv_len,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("TokenPrivileges query failed: error {err}"));
    }

    let (privilege_count, enabled_privilege_luids) = unsafe {
        validate_token_privileges_buffer(priv_buf.as_ptr(), priv_len, priv_buf.byte_capacity())?
    };
    evidence.privilege_count = privilege_count;
    evidence.enabled_privilege_count = enabled_privilege_luids.len() as u32;
    evidence.enabled_privilege_luids = enabled_privilege_luids;

    // 4. Verify TokenIntegrityLevel
    let mut int_len: u32 = 0;
    unsafe {
        GetTokenInformation(
            restricted_token,
            TokenIntegrityLevel,
            null_mut(),
            0,
            &mut int_len,
        );
    }
    if int_len == 0 {
        return Err("TokenIntegrityLevel query returned 0 length".into());
    }
    let mut int_buf = AlignedBuffer::new(int_len as usize);
    let ok = unsafe {
        GetTokenInformation(
            restricted_token,
            TokenIntegrityLevel,
            int_buf.as_mut_ptr() as _,
            int_buf.byte_capacity() as u32,
            &mut int_len,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("TokenIntegrityLevel query failed: error {err}"));
    }
    let (integrity_sid, integrity_rid) = unsafe {
        validate_token_integrity_buffer(int_buf.as_ptr(), int_len, int_buf.byte_capacity())?
    };
    if !is_canonical_sid(&integrity_sid) {
        return Err("TokenIntegrityLevel SID was not canonical".into());
    }
    evidence.integrity_level_rid = integrity_rid;

    Ok((restricted_guard, evidence))
}

/// Structured evidence collected from inside the restricted child process.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ChildProbeEvidence {
    pub child_execution_verified: bool,
    pub controller_vm_write: OperationOutcome,
    pub controller_terminate: OperationOutcome,
    pub controller_dup_handle: OperationOutcome,
    pub controller_query_info: OperationOutcome,
    pub controller_token_duplicate: OperationOutcome,
    pub candidate_create_ok: bool,
    pub candidate_append_ok: bool,
    pub candidate_rename_ok: bool,
    pub candidate_delete_ok: bool,
    pub scratch_create_ok: bool,
    pub scratch_delete_ok: bool,
    pub primary_create: OperationOutcome,
    pub primary_append: OperationOutcome,
    pub primary_overwrite: OperationOutcome,
    pub primary_delete: OperationOutcome,
    pub primary_rename: OperationOutcome,
    pub primary_mkdir: OperationOutcome,
    pub engine_create: OperationOutcome,
    pub engine_overwrite: OperationOutcome,
    pub sentinels_overwrite: OperationOutcome,
    pub outside_overwrite: OperationOutcome,
    pub traversal_write: OperationOutcome,
}

impl ChildProbeEvidence {
    pub(crate) fn is_all_passed(&self) -> bool {
        self.child_execution_verified
            && self.controller_vm_write.is_denied()
            && self.controller_terminate.is_denied()
            && self.controller_dup_handle.is_denied()
            && self.controller_token_duplicate.is_denied()
            && self.candidate_create_ok
            && self.candidate_append_ok
            && self.candidate_rename_ok
            && self.candidate_delete_ok
            && self.scratch_create_ok
            && self.scratch_delete_ok
            && self.primary_create.is_denied()
            && self.primary_append.is_denied()
            && self.primary_overwrite.is_denied()
            && self.primary_delete.is_denied()
            && self.primary_rename.is_denied()
            && self.primary_mkdir.is_denied()
            && self.engine_create.is_denied()
            && self.engine_overwrite.is_denied()
            && self.sentinels_overwrite.is_denied()
            && self.outside_overwrite.is_denied()
            && self.traversal_write.is_denied()
    }
}

/// Launches the restricted child process:
/// - Exact fully qualified libtest name with `--exact`.
/// - Direct launch inherits zero handles; the explicit CDB branch inherits only its
///   NUL stdin and bounded diagnostic pipe through `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
/// - Environment block sorted ASCII case-insensitively.
/// - Created `CREATE_SUSPENDED`.
/// - Assigned to `PocJobObject` before resuming.
/// - Resumed and waited on with timeout.
fn launch_poc_restricted_child(
    restricted_token: HANDLE,
    window_station_name: &str,
    desktop_name: &str,
    job: &PocJobObject,
    fixture: &PocFixture,
    controller_pid: u32,
    cdb_path: Option<&Path>,
    cdb_report: &mut Option<PendingCdbReport>,
) -> PocResult<ChildProbeEvidence> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("cannot get current_exe: {e}"))?;
    let child_command = build_child_test_command(&current_exe);
    let diagnostic_log = if cdb_path.is_some() {
        let dir = fixture
            .candidate_dir
            .join(format!("cdb-diagnostic-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir)
            .map_err(|e| format!("cannot create unique CDB diagnostic directory: {e}"))?;
        let metadata = std::fs::symlink_metadata(&dir)
            .map_err(|e| format!("cannot inspect CDB diagnostic directory: {e}"))?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("CDB diagnostic directory is a reparse point".into());
            }
        }
        if metadata.file_type().is_symlink() {
            return Err("CDB diagnostic directory is a symlink".into());
        }
        Some(dir.join("trace.log"))
    } else {
        None
    };
    let cmd_line = select_poc_child_command(cdb_path, diagnostic_log.as_deref(), &child_command)?;
    let mut cmd_line_w = to_wide_null(&cmd_line);
    let lp_desktop = build_poc_lp_desktop_name(window_station_name, desktop_name)?;
    let mut desktop_w = to_wide_null(&lp_desktop);
    let mut cdb_capture = if cdb_path.is_some() {
        Some(CdbPipeCapture::new()?)
    } else {
        None
    };

    let env_pairs = poc_child_environment_pairs(fixture, controller_pid);
    let mut env_block = create_unicode_environment_block(&env_pairs)?;

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.lpDesktop = desktop_w.as_mut_ptr();
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = 0; // SW_HIDE
    let mut siex: Option<STARTUPINFOEXW> = cdb_capture.as_ref().map(|capture| {
        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.lpDesktop = desktop_w.as_mut_ptr();
        startup.StartupInfo.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES;
        startup.StartupInfo.wShowWindow = 0;
        startup.StartupInfo.hStdInput = capture.stdin.as_ref().unwrap().0;
        startup.StartupInfo.hStdOutput = capture.write.as_ref().unwrap().0;
        startup.StartupInfo.hStdError = capture.write.as_ref().unwrap().0;
        startup.lpAttributeList = capture.attributes.ptr;
        startup
    });

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let creation_flags = CREATE_SUSPENDED
        | CREATE_UNICODE_ENVIRONMENT
        | if cdb_capture.is_some() {
            EXTENDED_STARTUPINFO_PRESENT
        } else {
            0
        };

    let application_name = cdb_path.map(|p| to_wide_null(p));
    if let Some(report) = cdb_report.as_mut() {
        report.target_image = Some(cdb_report_image_name(&current_exe));
        report.classification = CdbTraceClassification::DebuggerFailed;
    }
    let ok = unsafe {
        CreateProcessAsUserW(
            restricted_token,
            application_name.as_ref().map_or(null(), |v| v.as_ptr()),
            cmd_line_w.as_mut_ptr(),
            null(),
            null(),
            if cdb_capture.is_some() { 1 } else { FALSE },
            creation_flags,
            env_block.as_mut_ptr() as _,
            null(),
            siex.as_mut()
                .map_or(&mut si, |startup| &mut startup.StartupInfo),
            &mut pi,
        )
    };

    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("CreateProcessAsUserW failed: error {err}"));
    }

    let hprocess_guard = PocHandleGuard(pi.hProcess);
    let hthread_guard = PocHandleGuard(pi.hThread);

    if let Err(error) = job.assign_process(pi.hProcess) {
        let terminated = unsafe { TerminateProcess(pi.hProcess, 1) } != 0;
        let reaped = unsafe { WaitForSingleObject(pi.hProcess, 5_000) } == 0;
        return Err(format!(
            "{error}; suspended CDB termination confirmed={terminated}; reaped={reaped}"
        ));
    }

    let mut cdb_capture_buffer = CdbCaptureBuffer::default();
    if let Some(capture) = cdb_capture.as_mut() {
        drop(capture.write.take());
        drop(capture.stdin.take());
    }
    let mut cap_termination_attempted = false;

    let prev = unsafe { ResumeThread(pi.hThread) };
    if prev == u32::MAX {
        let err = unsafe { GetLastError() };
        if cdb_path.is_some() {
            let _ = unsafe { TerminateJobObject(job.handle.0, 1) };
            let _ = unsafe { WaitForSingleObject(pi.hProcess, 5_000) };
            let _ = wait_for_job_empty(job.handle.0, std::time::Duration::from_secs(5));
        }
        return Err(format!("ResumeThread failed: error {err}"));
    }

    let started = std::time::Instant::now();
    loop {
        if let Some(capture) = cdb_capture.as_ref() {
            let read_handle = capture
                .read
                .as_ref()
                .ok_or_else(|| "CDB output pipe reader handle is unavailable".to_string())?
                .0;
            if let Err(error) = drain_cdb_pipe(read_handle, &mut cdb_capture_buffer) {
                let _ = unsafe { TerminateJobObject(job.handle.0, 1) };
                let _ = unsafe { WaitForSingleObject(pi.hProcess, 5_000) };
                let _ = wait_for_job_empty(job.handle.0, std::time::Duration::from_secs(5));
                let persist = persist_cdb_capture(diagnostic_log.as_deref(), &cdb_capture_buffer);
                if let Some(report) = cdb_report.as_mut() {
                    report.observe(
                        Some(&current_exe),
                        None,
                        &cdb_capture_buffer.bytes,
                        true,
                        CdbTraceClassification::DebuggerFailed,
                    );
                }
                return Err(format!(
                    "CDB pipe capture failed: {error}; partial bounded trace persistence={persist:?}; final classification deferred until Window Station restoration; preserve fixture"
                ));
            }
            if cdb_capture_buffer.exceeded && !cap_termination_attempted {
                cap_termination_attempted = true;
                if unsafe { TerminateJobObject(job.handle.0, 1) } == 0 {
                    let error = unsafe { GetLastError() };
                    if let Some(report) = cdb_report.as_mut() {
                        report.observe(
                            Some(&current_exe),
                            None,
                            &cdb_capture_buffer.bytes,
                            true,
                            CdbTraceClassification::TraceTruncated,
                        );
                    }
                    return Err(format!(
                        "CDB output cap exceeded; TerminateJobObject failed: error {error}; final classification deferred until Window Station restoration; fixture must be preserved"
                    ));
                }
            }
        }
        let wait_res = unsafe { WaitForSingleObject(pi.hProcess, 100) };
        if wait_res == 0 {
            break;
        }
        if wait_res != 0x00000102 {
            if cdb_path.is_some() {
                let _ = unsafe { TerminateJobObject(job.handle.0, 1) };
                let _ = unsafe { WaitForSingleObject(pi.hProcess, 5_000) };
                let _ = wait_for_job_empty(job.handle.0, std::time::Duration::from_secs(5));
                if let Some(report) = cdb_report.as_mut() {
                    report.observe(
                        Some(&current_exe),
                        None,
                        &cdb_capture_buffer.bytes,
                        true,
                        CdbTraceClassification::TraceTruncated,
                    );
                }
            }
            return Err(format!("child process wait failed: wait result {wait_res}; CDB classification deferred until Window Station restoration"));
        }
        if started.elapsed() >= std::time::Duration::from_secs(30) {
            if cdb_path.is_some() {
                let terminated = unsafe { TerminateJobObject(job.handle.0, 1) } != 0;
                let reaped = unsafe { WaitForSingleObject(pi.hProcess, 5_000) } == 0;
                let job_empty = wait_for_job_empty(job.handle.0, std::time::Duration::from_secs(5));
                if !terminated || !reaped {
                    if let Some(report) = cdb_report.as_mut() {
                        report.observe(
                            Some(&current_exe),
                            None,
                            &cdb_capture_buffer.bytes,
                            true,
                            CdbTraceClassification::TraceTruncated,
                        );
                    }
                    return Err("CDB timeout; debugger termination/reap was not confirmed; preserve fixture".into());
                }
                if let Err(error) = job_empty {
                    if let Some(report) = cdb_report.as_mut() {
                        report.observe(
                            Some(&current_exe),
                            None,
                            &cdb_capture_buffer.bytes,
                            true,
                            CdbTraceClassification::TraceTruncated,
                        );
                    }
                    return Err(format!(
                        "CDB timeout; process-tree reaping unverified: {error}; preserve fixture"
                    ));
                }
                let persist = persist_cdb_capture(diagnostic_log.as_deref(), &cdb_capture_buffer);
                let summary =
                    summarize_cdb_trace(&String::from_utf8_lossy(&cdb_capture_buffer.bytes), true);
                if let Some(report) = cdb_report.as_mut() {
                    report.observe(
                        Some(&current_exe),
                        None,
                        &cdb_capture_buffer.bytes,
                        true,
                        CdbTraceClassification::TraceTruncated,
                    );
                }
                return Err(format!("CDB timed out after 30 seconds; partial trace summary={summary:?}; persistence={persist:?}; final classification deferred until Window Station restoration; fixture must be preserved"));
            }
            return Err("child/debugger failed to complete within 30-second timeout; fixture must be preserved".into());
        }
    }

    if let Some(capture) = cdb_capture.as_ref() {
        let read_handle = capture.read.as_ref().unwrap().0;
        if let Err(error) = drain_cdb_pipe(read_handle, &mut cdb_capture_buffer) {
            if let Some(report) = cdb_report.as_mut() {
                report.observe(
                    Some(&current_exe),
                    None,
                    &cdb_capture_buffer.bytes,
                    true,
                    CdbTraceClassification::DebuggerFailed,
                );
            }
            return Err(format!("final CDB pipe drain failed: {error}; final classification deferred until Window Station restoration; preserve fixture"));
        }
    }
    let capture = cdb_path.is_some().then_some(cdb_capture_buffer);

    if let (Some(log), Some(capture)) = (diagnostic_log.as_deref(), capture.as_ref()) {
        if let Some(report) = cdb_report.as_mut() {
            report.observe(
                Some(&current_exe),
                None,
                capture.bytes.as_slice(),
                capture.exceeded,
                CdbTraceClassification::TargetExitUnobserved,
            );
        }
        // Persist the already bounded bytes before any later fallible reporting step.
        // A persistence failure is itself reported after ACL restoration by the caller.
        if let Err(error) = persist_cdb_capture(Some(log), capture) {
            return Err(format!(
                "CDB trace persistence failed: {error}; final classification deferred until Window Station restoration"
            ));
        }
    }

    if cdb_path.is_some() {
        if let Err(error) = wait_for_job_empty(job.handle.0, std::time::Duration::from_secs(5)) {
            if let Some(report) = cdb_report.as_mut() {
                if let Some(capture) = capture.as_ref() {
                    report.observe(
                        Some(&current_exe),
                        None,
                        capture.bytes.as_slice(),
                        true,
                        CdbTraceClassification::TraceTruncated,
                    );
                }
            }
            return Err(format!(
                "CDB process-tree completion unverified: {error}; final classification deferred until Window Station restoration; preserve fixture"
            ));
        }
    }

    if diagnostic_log.is_some() {
        let capture = capture
            .as_ref()
            .ok_or_else(|| "CDB diagnostic output capture is unavailable".to_string())?;
        if capture.exceeded {
            if let Some(report) = cdb_report.as_mut() {
                report.observe(
                    Some(&current_exe),
                    None,
                    &capture.bytes,
                    true,
                    CdbTraceClassification::TraceTruncated,
                );
            }
            return Err(format!(
                "CDB trace exceeded {} bytes; final classification deferred until Window Station restoration; fixture must be preserved; bounded trace persistence was attempted",
                CDB_LOG_LIMIT_BYTES
            ));
        }
    }

    let mut exit_code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(pi.hProcess, &mut exit_code) };
    if ok == 0 {
        return Err(format!(
            "child process GetExitCodeProcess failed: error {}",
            unsafe { GetLastError() }
        ));
    }
    if diagnostic_log.is_some() {
        let capture = capture
            .as_ref()
            .ok_or_else(|| "CDB output capture was not completed".to_string())?;
        if let Some(report) = cdb_report.as_mut() {
            report.observe(
                Some(&current_exe),
                Some(exit_code),
                capture.bytes.as_slice(),
                false,
                CdbTraceClassification::TargetExitedUnidentified,
            );
        }
    }
    if exit_code != 0 {
        if diagnostic_log.is_some() {
            let trace = String::from_utf8_lossy(
                &capture
                    .as_ref()
                    .map(|c| c.bytes.as_slice())
                    .unwrap_or_default(),
            );
            return Err(format!("CDB diagnostic classification={:?}; debugger_exit={exit_code}; summary={:?}; bounded trace persisted; target evidence unavailable", classify_cdb_trace(&trace, false, false), summarize_cdb_trace(&trace, false)));
        }
        return Err(format!(
            "child process exited with non-zero exit code: {}",
            format_poc_child_exit_status(exit_code)
        ));
    }

    drop(hthread_guard);
    drop(hprocess_guard);

    let evidence_path = fixture.candidate_dir.join("child_evidence.json");
    let evidence_bytes = std::fs::read(&evidence_path).map_err(|e| {
        if diagnostic_log.is_some() {
            let trace = String::from_utf8_lossy(
                &capture.as_ref().map(|c| c.bytes.as_slice()).unwrap_or_default(),
            );
            return format!(
                "CDB diagnostic classification={:?}; summary={:?}; bounded trace persisted; target evidence unavailable: {e}",
                classify_cdb_trace(&trace, false, false),
                summarize_cdb_trace(&trace, false)
            );
        }
        format!(
            "cannot read child evidence from '{}': {e}",
            evidence_path.display()
        )
    })?;
    let evidence: ChildProbeEvidence = serde_json::from_slice(&evidence_bytes)
        .map_err(|e| format!("cannot parse child evidence: {e}"))?;

    Ok(evidence)
}

/// Implementation of the child-side probe execution.
/// Called exclusively when `POVERELLO_POC_CHILD_ROLE == 1`.
pub(crate) fn run_child_probe_implementation() -> PocResult<()> {
    let controller_pid: u32 = std::env::var("POVERELLO_POC_CONTROLLER_PID")
        .map_err(|_| "missing POVERELLO_POC_CONTROLLER_PID".to_string())?
        .parse()
        .map_err(|e| format!("invalid controller pid: {e}"))?;

    let candidate_dir = PathBuf::from(std::env::var("POVERELLO_POC_CANDIDATE_DIR").unwrap());
    let scratch_dir = PathBuf::from(std::env::var("POVERELLO_POC_SCRATCH_DIR").unwrap());
    let primary_dir = PathBuf::from(std::env::var("POVERELLO_POC_PRIMARY_DIR").unwrap());
    let engine_dir = PathBuf::from(std::env::var("POVERELLO_POC_ENGINE_DIR").unwrap());
    let sentinels_dir = PathBuf::from(std::env::var("POVERELLO_POC_SENTINELS_DIR").unwrap());
    let outside_dir = PathBuf::from(std::env::var("POVERELLO_POC_OUTSIDE_DIR").unwrap());

    let mut evidence = ChildProbeEvidence {
        child_execution_verified: true,
        ..Default::default()
    };

    // 1. Controller process rights probes (individual independent checks)
    let h_vm = unsafe { OpenProcess(PROCESS_VM_WRITE, FALSE, controller_pid) };
    evidence.controller_vm_write = if h_vm.is_null() {
        let err = unsafe { GetLastError() };
        if err == ERROR_ACCESS_DENIED {
            OperationOutcome::DeniedAccessDenied
        } else {
            OperationOutcome::FailedOther(err as i32)
        }
    } else {
        unsafe { CloseHandle(h_vm) };
        OperationOutcome::Allowed
    };

    let h_term = unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, controller_pid) };
    evidence.controller_terminate = if h_term.is_null() {
        let err = unsafe { GetLastError() };
        if err == ERROR_ACCESS_DENIED {
            OperationOutcome::DeniedAccessDenied
        } else {
            OperationOutcome::FailedOther(err as i32)
        }
    } else {
        unsafe { CloseHandle(h_term) };
        OperationOutcome::Allowed
    };

    let h_dup = unsafe { OpenProcess(PROCESS_DUP_HANDLE, FALSE, controller_pid) };
    evidence.controller_dup_handle = if h_dup.is_null() {
        let err = unsafe { GetLastError() };
        if err == ERROR_ACCESS_DENIED {
            OperationOutcome::DeniedAccessDenied
        } else {
            OperationOutcome::FailedOther(err as i32)
        }
    } else {
        unsafe { CloseHandle(h_dup) };
        OperationOutcome::Allowed
    };

    // Independent check A: OpenProcess(PROCESS_QUERY_INFORMATION)
    let h_query = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, controller_pid) };
    evidence.controller_query_info = if h_query.is_null() {
        let err = unsafe { GetLastError() };
        if err == ERROR_ACCESS_DENIED {
            OperationOutcome::DeniedAccessDenied
        } else {
            OperationOutcome::FailedOther(err as i32)
        }
    } else {
        unsafe { CloseHandle(h_query) };
        OperationOutcome::Allowed
    };

    // Independent check B: OpenProcessToken(TOKEN_DUPLICATE) on process handle
    // Try opening process with limited query right to get a handle for OpenProcessToken
    let h_proc_limited =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, controller_pid) };
    if !h_proc_limited.is_null() {
        let mut h_token: HANDLE = null_mut();
        let ok = unsafe {
            OpenProcessToken(
                h_proc_limited,
                windows_sys::Win32::Security::TOKEN_QUERY
                    | windows_sys::Win32::Security::TOKEN_DUPLICATE,
                &mut h_token,
            )
        };
        let token_res = if ok == 0 {
            let err = unsafe { GetLastError() };
            Err(err)
        } else {
            unsafe { CloseHandle(h_token) };
            Ok(())
        };
        unsafe { CloseHandle(h_proc_limited) };
        evidence.controller_token_duplicate =
            map_process_and_token_open_outcome(Ok(()), Some(token_res));
    } else {
        // Prerequisite OpenProcess failed; OpenProcessToken was never called.
        // Record NotAttempted so this prerequisite failure is never mistaken for token denial.
        let err = unsafe { GetLastError() };
        evidence.controller_token_duplicate = map_process_and_token_open_outcome(Err(err), None);
    }

    // 2. Candidate positive write tests
    let cand_file = candidate_dir.join("child_test.txt");
    if std::fs::write(&cand_file, b"child positive candidate create\n").is_ok() {
        evidence.candidate_create_ok = true;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&cand_file) {
        if f.write_all(b"appended\n").is_ok() {
            evidence.candidate_append_ok = true;
        }
    }
    let cand_renamed = candidate_dir.join("child_test_renamed.txt");
    if std::fs::rename(&cand_file, &cand_renamed).is_ok() {
        evidence.candidate_rename_ok = true;
        if std::fs::remove_file(&cand_renamed).is_ok() {
            evidence.candidate_delete_ok = true;
        }
    }

    // Scratch positive write tests
    let scratch_file = scratch_dir.join("child_scratch.txt");
    if std::fs::write(&scratch_file, b"child positive scratch\n").is_ok() {
        evidence.scratch_create_ok = true;
        if std::fs::remove_file(&scratch_file).is_ok() {
            evidence.scratch_delete_ok = true;
        }
    }

    // 3. Negative matrix against protected trees using separate canaries
    let prim_new = primary_dir.join("forbidden_new.txt");
    let res = std::fs::write(&prim_new, b"forbidden\n");
    evidence.primary_create = OperationOutcome::from_io_result(&res);

    let prim_append = primary_dir.join("append_canary.rs");
    let res = std::fs::OpenOptions::new()
        .append(true)
        .open(&prim_append)
        .and_then(|mut f| f.write_all(b"appended\n"));
    evidence.primary_append = OperationOutcome::from_io_result(&res);

    let prim_overwrite = primary_dir.join("overwrite_canary.rs");
    let res = std::fs::write(&prim_overwrite, b"overwritten\n");
    evidence.primary_overwrite = OperationOutcome::from_io_result(&res);

    let prim_delete = primary_dir.join("delete_canary.rs");
    let res = std::fs::remove_file(&prim_delete);
    evidence.primary_delete = OperationOutcome::from_io_result(&res);

    let prim_rename = primary_dir.join("rename_canary.rs");
    let res = std::fs::rename(&prim_rename, primary_dir.join("renamed_canary.rs"));
    evidence.primary_rename = OperationOutcome::from_io_result(&res);

    let prim_mkdir = primary_dir.join("new_forbidden_subdir");
    let res = std::fs::create_dir(&prim_mkdir);
    evidence.primary_mkdir = OperationOutcome::from_io_result(&res);

    let eng_new = engine_dir.join("forbidden_state.json");
    let res = std::fs::write(&eng_new, b"forbidden state");
    evidence.engine_create = OperationOutcome::from_io_result(&res);

    let eng_overwrite = engine_dir.join("engine_overwrite_canary.json");
    let res = std::fs::write(&eng_overwrite, b"{\"corrupted\": true}");
    evidence.engine_overwrite = OperationOutcome::from_io_result(&res);

    let sent_overwrite = sentinels_dir.join("untracked_scratch.txt");
    let res = std::fs::write(&sent_overwrite, b"corrupted untracked sentinel");
    evidence.sentinels_overwrite = OperationOutcome::from_io_result(&res);

    let out_overwrite = outside_dir.join("outside_canary.txt");
    let res = std::fs::write(&out_overwrite, b"corrupted outside canary");
    evidence.outside_overwrite = OperationOutcome::from_io_result(&res);

    let trav_path = candidate_dir.join("../primary/traversal_attack.txt");
    let res = std::fs::write(&trav_path, b"traversal escape\n");
    evidence.traversal_write = OperationOutcome::from_io_result(&res);

    let evidence_path = candidate_dir.join("child_evidence.json");
    let ser = serde_json::to_vec_pretty(&evidence).unwrap();
    std::fs::write(evidence_path, ser).map_err(|e| format!("cannot write evidence: {e}"))?;

    Ok(())
}

fn sha256_digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(s, "{byte:02x}");
    }
    s
}

// ---------------------------------------------------------------------------
// Pure Deterministic Offline Tests (No processes, tokens, desktops, ACLs, or Job Objects)
// ---------------------------------------------------------------------------

#[test]
fn test_poc_opt_in_parser_matrix() {
    assert!(!evaluate_poc_opt_in(None));
    assert!(!evaluate_poc_opt_in(Some("")));
    assert!(!evaluate_poc_opt_in(Some("0")));
    assert!(!evaluate_poc_opt_in(Some("false")));
    assert!(!evaluate_poc_opt_in(Some("true")));
    assert!(!evaluate_poc_opt_in(Some("2")));
    assert!(!evaluate_poc_opt_in(Some("yes")));
    assert!(evaluate_poc_opt_in(Some("1")));
}

#[test]
fn test_poc_cdb_command_is_noninteractive_and_targets_child() {
    let command = build_cdb_child_command(
        Path::new(r"C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe"),
        Path::new(r"C:\temp\diag\cdb.log"),
        r#""C:\repo\test.exe" test_name --exact --ignored --nocapture"#,
    );
    assert!(!command.contains("-logo"));
    assert!(command.contains("-c \"sxn ld;"));
    assert!(command.contains("sxe 0xc0000142"));
    assert!(command.contains("sxe 0xc0000139"));
    assert!(command.contains("sxn ld"));
    assert!(command.contains(".lastevent; .exr -1; .ecxr; ln @$ip; lm a @$ip"));
    assert!(command.ends_with(r#""C:\repo\test.exe" test_name --exact --ignored --nocapture"#));
}

#[test]
fn test_poc_cdb_default_launch_command_is_unchanged_and_pairing_is_required() {
    let child = r#""C:\repo\test.exe" test_name --exact"#;
    assert_eq!(select_poc_child_command(None, None, child).unwrap(), child);
    assert!(select_poc_child_command(Some(Path::new(r"C:\kits\cdb.exe")), None, child).is_err());
    assert!(select_poc_child_command(None, Some(Path::new(r"C:\temp\trace.log")), child).is_err());
}

#[test]
fn test_poc_cdb_opt_in_and_trace_classification_fail_closed() {
    assert_eq!(resolve_cdb_diagnostic_path(None, None).unwrap(), None);
    assert!(resolve_cdb_diagnostic_path(Some("1"), None).is_err());
    assert!(resolve_cdb_diagnostic_path(Some("true"), None).is_err());
    assert!(resolve_cdb_diagnostic_path(None, Some(Path::new(r"C:\cdb.exe"))).is_err());
    assert_eq!(
        classify_cdb_trace("Exception 0xc0000142", false, false),
        CdbTraceClassification::ObservedDllInitFailedStatus
    );
    assert_eq!(
        classify_cdb_trace("path contains c0000142 but no debugger event", false, false),
        CdbTraceClassification::TargetExitedUnidentified
    );
    assert_eq!(
        classify_cdb_trace("Exception c0000139", false, false),
        CdbTraceClassification::ObservedEntryPointNotFoundStatus
    );
    assert_eq!(
        classify_cdb_trace("incomplete", true, false),
        CdbTraceClassification::TraceTruncated
    );
    assert_eq!(
        classify_cdb_trace("Exception 0xc0000142", false, true),
        CdbTraceClassification::AclRestoreFailed
    );
    assert_eq!(
        classify_cdb_trace("process exited", false, false),
        CdbTraceClassification::TargetExitedUnidentified
    );
}

#[test]
fn test_poc_cdb_trace_summary_is_bounded_and_redacts_module_directories() {
    let trace = "Microsoft Debugger Version 10.0\nModLoad: 0001 0002 C:\\Windows\\System32\\kernelbase.dll\nException c0000142\nchild.exe exited with code 0xc0000142\n";
    let summary = summarize_cdb_trace(trace, false);
    assert_eq!(
        summary.debugger_version_line.as_deref(),
        Some("Microsoft Debugger Version 10.0")
    );
    assert_eq!(summary.module_events, vec!["kernelbase.dll"]);
    assert_eq!(summary.observed_statuses, vec!["c0000142"]);
    assert_eq!(
        summary.target_exit_line.as_deref(),
        Some("target exited with code 0xc0000142")
    );
    assert_eq!(
        summary
            .ordered_events
            .iter()
            .map(|e| e.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(!format!("{summary:?}").contains("Windows\\System32"));
    assert!(summarize_cdb_trace(trace, true).truncated);
}

#[test]
fn test_poc_cdb_debugger_version_summary_stops_before_drive_and_unc_paths() {
    let drive_path = summarize_cdb_trace(
        "Microsoft Debugger Version 10.0.1 C:\\Users\\private-user\\cdb.exe\n",
        false,
    );
    assert_eq!(
        drive_path.debugger_version_line.as_deref(),
        Some("Microsoft Debugger Version 10.0.1")
    );
    assert!(!format!("{drive_path:?}").contains("private-user"));

    let unc_path = summarize_cdb_trace(
        "Microsoft Debugger Version 10.0.1 \\\\server\\share\\cdb.exe\n",
        false,
    );
    assert_eq!(
        unc_path.debugger_version_line.as_deref(),
        Some("Microsoft Debugger Version 10.0.1")
    );
    assert!(!format!("{unc_path:?}").contains("server"));
}

#[test]
fn test_poc_cdb_target_exit_summary_accepts_only_bounded_status_tokens() {
    let decimal = summarize_cdb_trace(
        "child.exe exited with code 17 C:\\Users\\private-user\\extra.txt\n",
        false,
    );
    assert_eq!(
        decimal.target_exit_line.as_deref(),
        Some("target exited with code 17")
    );
    assert!(!format!("{decimal:?}").contains("private-user"));

    let hexadecimal = summarize_cdb_trace(
        "child.exe exited with code 0xc0000142 \\\\server\\share\\extra.txt\n",
        false,
    );
    assert_eq!(
        hexadecimal.target_exit_line.as_deref(),
        Some("target exited with code 0xc0000142")
    );
    assert!(!format!("{hexadecimal:?}").contains("server"));

    let malformed = summarize_cdb_trace(
        "child.exe exited with code C:\\Users\\private-user\\child.exe\n",
        false,
    );
    assert_eq!(malformed.target_exit_line, None);
    assert!(malformed.ordered_events.is_empty());
    assert!(!format!("{malformed:?}").contains("private-user"));
}

#[test]
fn test_poc_cdb_capture_buffer_never_exceeds_fixed_limit() {
    let mut capture = CdbCaptureBuffer::default();
    let full = vec![b'x'; CDB_LOG_LIMIT_BYTES as usize];
    capture.push(&full);
    assert_eq!(capture.bytes.len(), CDB_LOG_LIMIT_BYTES as usize);
    assert!(!capture.exceeded);

    capture.push(b"y");
    assert_eq!(capture.bytes.len(), CDB_LOG_LIMIT_BYTES as usize);
    assert!(capture.exceeded);
}

#[test]
fn test_poc_cdb_report_redacts_absolute_paths_but_keeps_basename() {
    let mut report = PendingCdbReport::new("run-redaction-test");
    report.observe(
        Some(Path::new(
            r"C:\Users\private-user\checkout\target\poc-child.exe",
        )),
        Some(1),
        b"Exception 0xc0000142",
        false,
        CdbTraceClassification::DebuggerFailed,
    );

    let rendered = report.render();
    assert_eq!(report.target_image.as_deref(), Some("poc-child.exe"));
    assert!(rendered.contains("poc-child.exe"));
    assert!(!rendered.contains("private-user"));
}

#[test]
fn test_poc_cdb_persistence_error_does_not_add_a_path() {
    let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    let rendered = format_cdb_persist_error(&error);
    assert!(rendered.contains("cannot persist bounded CDB trace"));
    assert!(!rendered.contains(r"C:\Users\private-user"));
}

#[test]
fn test_poc_cdb_acl_restore_failure_overrides_final_report_classification() {
    let mut report = Some(PendingCdbReport::new("cdb-diagnostic-test"));
    report.as_mut().unwrap().observe(
        Some(Path::new("test.exe")),
        Some(0),
        b"target exited with code 0",
        false,
        CdbTraceClassification::TargetExitedUnidentified,
    );
    assert_eq!(
        report.as_ref().unwrap().classification,
        CdbTraceClassification::TargetExitedUnidentified
    );
    apply_cdb_acl_restore_result(&mut report, true);
    assert_eq!(
        report.unwrap().classification,
        CdbTraceClassification::AclRestoreFailed
    );
}

#[test]
fn test_poc_cdb_pending_report_records_failure_truncation_and_normal_observations() {
    let mut report = PendingCdbReport::new("run-123");
    assert_eq!(
        report.classification,
        CdbTraceClassification::DiagnosticNotStarted
    );
    assert_eq!(report.target_image, None);
    assert_eq!(report.debugger_exit, None);

    report.observe(
        Some(Path::new(r"C:\target.exe")),
        None,
        b"",
        false,
        CdbTraceClassification::DebuggerFailed,
    );
    assert_eq!(
        report.classification,
        CdbTraceClassification::DebuggerFailed
    );
    assert_eq!(report.target_image.as_deref(), Some("target.exe"));

    report.observe(
        None,
        None,
        b"Exception 0xc0000142",
        true,
        CdbTraceClassification::DebuggerFailed,
    );
    assert_eq!(
        report.classification,
        CdbTraceClassification::TraceTruncated
    );
    assert!(report.summary.truncated);

    report.observe(
        None,
        Some(0),
        b"Microsoft (R) Windows Debugger Version 10.0",
        false,
        CdbTraceClassification::DebuggerFailed,
    );
    assert_eq!(
        report.classification,
        CdbTraceClassification::TargetExitUnobserved
    );

    report.observe(
        None,
        Some(0),
        b"Exception 0xc0000142\nchild exited with code 0",
        false,
        CdbTraceClassification::TargetExitedUnidentified,
    );
    assert_eq!(
        report.classification,
        CdbTraceClassification::ObservedDllInitFailedStatus
    );
    assert_eq!(report.debugger_exit, Some(0));
}

#[test]
fn test_poc_cdb_path_and_log_size_pure_boundaries() {
    let root = Path::new(r"C:\Kits\Debuggers\x64");
    assert!(validate_cdb_canonical_path(&root.join("cdb.exe"), root).is_ok());
    assert!(validate_cdb_canonical_path(&root.join("other.exe"), root).is_err());
    assert!(validate_cdb_canonical_path(Path::new(r"C:\outside\cdb.exe"), root).is_err());
    assert!(!cdb_log_exceeds_limit(CDB_LOG_LIMIT_BYTES));
    assert!(cdb_log_exceeds_limit(CDB_LOG_LIMIT_BYTES + 1));

    let windows_kits_root = Path::new(r"C:\Program Files (x86)\Windows Kits\10\Debuggers\x64");
    let cdb = windows_kits_root.join("cdb.exe");
    let junction_ancestor = windows_kits_root.parent().unwrap().to_path_buf();
    assert!(
        validate_cdb_path_with_meta(&cdb, windows_kits_root, |component| {
            Ok(PocFileMetadata {
                is_symlink: component == junction_ancestor,
                is_reparse_point: component == junction_ancestor,
            })
        })
        .is_err()
    );
}

#[test]
fn test_poc_restriction_subauthorities_derivation_pure() {
    let u1 = uuid::Uuid::new_v4();
    let u2 = uuid::Uuid::new_v4();

    let s1 = derive_restriction_subauthorities(u1);
    let s2 = derive_restriction_subauthorities(u2);

    assert_ne!(
        s1, s2,
        "different UUIDs must yield different subauthorities"
    );

    let b1 = u1.as_bytes();
    let expected_s0 = u32::from_ne_bytes([b1[0], b1[1], b1[2], b1[3]]);
    assert_eq!(s1[0], expected_s0);
}

#[test]
fn test_poc_child_command_construction_pure() {
    let exe = Path::new("C:\\fake\\target\\debug\\deps\\test_runner.exe");
    let cmd = build_child_test_command(exe);

    assert!(cmd.starts_with("\"C:\\fake\\target\\debug\\deps\\test_runner.exe\""));
    assert!(cmd.contains(POC_CHILD_TEST_FULL_NAME));
    assert!(cmd.ends_with("--exact --ignored --nocapture"));
}

#[test]
fn test_poc_system_cmd_smoke_command_is_absolute_and_no_op() {
    let command = build_poc_system_cmd_smoke_command(Path::new("C:\\Windows"));
    assert_eq!(command, "\"C:\\Windows\\System32\\cmd.exe\" /d /c exit 0");
}

#[test]
fn test_poc_native_startup_helper_preflight_and_command() {
    assert!(require_poc_native_startup_helper_path(None).is_err());
    assert!(
        require_poc_native_startup_helper_path(Some(Path::new("relative\\helper.exe"))).is_err()
    );

    let absolute = Path::new("C:\\repo\\gui\\src-tauri\\target\\debug\\poc_startup_helper.exe");
    assert_eq!(
        require_poc_native_startup_helper_path(Some(absolute)).unwrap(),
        absolute
    );
    assert_eq!(
        build_poc_native_startup_helper_command(absolute),
        "\"C:\\repo\\gui\\src-tauri\\target\\debug\\poc_startup_helper.exe\""
    );

    let temp = tempfile::tempdir().unwrap();
    let target_root = temp.path().join("target");
    std::fs::create_dir(&target_root).unwrap();
    let helper = target_root.join("poc_startup_helper.exe");
    std::fs::write(&helper, b"offline path-validation fixture").unwrap();
    assert_eq!(
        validate_poc_native_startup_helper_path(&helper, &target_root).unwrap(),
        std::fs::canonicalize(&helper).unwrap()
    );

    let outside = temp.path().join("outside_helper.exe");
    std::fs::write(&outside, b"outside target root").unwrap();
    assert!(validate_poc_native_startup_helper_path(&outside, &target_root).is_err());
}

#[test]
fn test_poc_user32_startup_helper_preflight_and_command() {
    assert!(require_poc_user32_startup_helper_path(None).is_err());
    assert!(
        require_poc_user32_startup_helper_path(Some(Path::new("relative\\helper.exe"))).is_err()
    );

    let absolute =
        Path::new("C:\\repo\\gui\\src-tauri\\target\\debug\\poc_user32_startup_helper.exe");
    assert_eq!(
        require_poc_user32_startup_helper_path(Some(absolute)).unwrap(),
        absolute
    );
    assert_eq!(
        build_poc_native_startup_helper_command(absolute),
        "\"C:\\repo\\gui\\src-tauri\\target\\debug\\poc_user32_startup_helper.exe\""
    );

    let temp = tempfile::tempdir().unwrap();
    let target_root = temp.path().join("target");
    std::fs::create_dir(&target_root).unwrap();
    let helper = target_root.join("poc_user32_startup_helper.exe");
    std::fs::write(&helper, b"offline path-validation fixture").unwrap();
    assert_eq!(
        validate_poc_user32_startup_helper_path(&helper, &target_root).unwrap(),
        std::fs::canonicalize(&helper).unwrap()
    );

    let outside = temp.path().join("outside_helper.exe");
    std::fs::write(&outside, b"outside target root").unwrap();
    assert!(validate_poc_user32_startup_helper_path(&outside, &target_root).is_err());
}

#[test]
fn test_poc_userenv_startup_helper_preflight_and_command() {
    assert!(require_poc_userenv_startup_helper_path(None).is_err());
    assert!(
        require_poc_userenv_startup_helper_path(Some(Path::new("relative\\helper.exe"))).is_err()
    );

    let absolute =
        Path::new("C:\\repo\\gui\\src-tauri\\target\\debug\\poc_userenv_startup_helper.exe");
    assert_eq!(
        require_poc_userenv_startup_helper_path(Some(absolute)).unwrap(),
        absolute
    );
    assert_eq!(
        build_poc_native_startup_helper_command(absolute),
        "\"C:\\repo\\gui\\src-tauri\\target\\debug\\poc_userenv_startup_helper.exe\""
    );

    let temp = tempfile::tempdir().unwrap();
    let target_root = temp.path().join("target");
    std::fs::create_dir(&target_root).unwrap();
    let helper = target_root.join("poc_userenv_startup_helper.exe");
    std::fs::write(&helper, b"offline path-validation fixture").unwrap();
    assert_eq!(
        validate_poc_userenv_startup_helper_path(&helper, &target_root).unwrap(),
        std::fs::canonicalize(&helper).unwrap()
    );

    let outside = temp.path().join("outside_helper.exe");
    std::fs::write(&outside, b"outside target root").unwrap();
    assert!(validate_poc_userenv_startup_helper_path(&outside, &target_root).is_err());
}

#[test]
fn test_poc_vcruntime_startup_helper_preflight_and_command() {
    assert!(require_poc_vcruntime_startup_helper_path(None).is_err());
    assert!(
        require_poc_vcruntime_startup_helper_path(Some(Path::new("relative\\helper.exe"))).is_err()
    );

    let absolute =
        Path::new("C:\\repo\\gui\\src-tauri\\target\\debug\\poc_vcruntime_startup_helper.exe");
    assert_eq!(
        require_poc_vcruntime_startup_helper_path(Some(absolute)).unwrap(),
        absolute
    );
    assert_eq!(
        build_poc_native_startup_helper_command(absolute),
        "\"C:\\repo\\gui\\src-tauri\\target\\debug\\poc_vcruntime_startup_helper.exe\""
    );

    let temp = tempfile::tempdir().unwrap();
    let target_root = temp.path().join("target");
    std::fs::create_dir(&target_root).unwrap();
    let helper = target_root.join("poc_vcruntime_startup_helper.exe");
    std::fs::write(&helper, b"offline path-validation fixture").unwrap();
    assert_eq!(
        validate_poc_vcruntime_startup_helper_path(&helper, &target_root).unwrap(),
        std::fs::canonicalize(&helper).unwrap()
    );

    let outside = temp.path().join("outside_helper.exe");
    std::fs::write(&outside, b"outside target root").unwrap();
    assert!(validate_poc_vcruntime_startup_helper_path(&outside, &target_root).is_err());
}

#[test]
fn test_poc_crt_runtime_startup_helper_preflight_and_command() {
    assert!(require_poc_crt_runtime_startup_helper_path(None).is_err());
    assert!(
        require_poc_crt_runtime_startup_helper_path(Some(Path::new("relative\\helper.exe")))
            .is_err()
    );

    let absolute =
        Path::new("C:\\repo\\gui\\src-tauri\\target\\debug\\poc_crt_runtime_startup_helper.exe");
    assert_eq!(
        require_poc_crt_runtime_startup_helper_path(Some(absolute)).unwrap(),
        absolute
    );
    assert_eq!(
        build_poc_native_startup_helper_command(absolute),
        "\"C:\\repo\\gui\\src-tauri\\target\\debug\\poc_crt_runtime_startup_helper.exe\""
    );

    let temp = tempfile::tempdir().unwrap();
    let target_root = temp.path().join("target");
    std::fs::create_dir(&target_root).unwrap();
    let helper = target_root.join("poc_crt_runtime_startup_helper.exe");
    std::fs::write(&helper, b"offline path-validation fixture").unwrap();
    assert_eq!(
        validate_poc_crt_runtime_startup_helper_path(&helper, &target_root).unwrap(),
        std::fs::canonicalize(&helper).unwrap()
    );

    let outside = temp.path().join("outside_helper.exe");
    std::fs::write(&outside, b"outside target root").unwrap();
    assert!(validate_poc_crt_runtime_startup_helper_path(&outside, &target_root).is_err());
}

#[test]
fn test_poc_grouped_crt_startup_helper_preflight_and_command() {
    assert!(require_poc_grouped_crt_startup_helper_path(None).is_err());
    assert!(
        require_poc_grouped_crt_startup_helper_path(Some(Path::new("relative\\helper.exe")))
            .is_err()
    );

    let absolute =
        Path::new("C:\\repo\\gui\\src-tauri\\target\\debug\\poc_grouped_crt_startup_helper.exe");
    assert_eq!(
        require_poc_grouped_crt_startup_helper_path(Some(absolute)).unwrap(),
        absolute
    );
    assert_eq!(
        build_poc_native_startup_helper_command(absolute),
        "\"C:\\repo\\gui\\src-tauri\\target\\debug\\poc_grouped_crt_startup_helper.exe\""
    );

    let temp = tempfile::tempdir().unwrap();
    let target_root = temp.path().join("target");
    std::fs::create_dir(&target_root).unwrap();
    let helper = target_root.join("poc_grouped_crt_startup_helper.exe");
    std::fs::write(&helper, b"offline path-validation fixture").unwrap();
    assert_eq!(
        validate_poc_grouped_crt_startup_helper_path(&helper, &target_root).unwrap(),
        std::fs::canonicalize(&helper).unwrap()
    );

    let outside = temp.path().join("outside_helper.exe");
    std::fs::write(&outside, b"outside target root").unwrap();
    assert!(validate_poc_grouped_crt_startup_helper_path(&outside, &target_root).is_err());
}

#[test]
fn test_poc_grouped_crt_imports_def_vcruntime_spelling() {
    let def_content = include_str!("grouped_crt_imports.def");
    assert!(
        def_content.contains("_CxxThrowException"),
        "expected _CxxThrowException in grouped_crt_imports.def"
    );
    assert!(
        !def_content.contains("__CxxThrowException"),
        "unexpected __CxxThrowException in grouped_crt_imports.def"
    );
}

#[test]
fn test_poc_lp_desktop_name_is_explicit_and_rejects_path_components() {
    assert_eq!(
        build_poc_lp_desktop_name("WinSta0", "Poverello-PoC-Desktop-abc").unwrap(),
        "WinSta0\\Poverello-PoC-Desktop-abc"
    );
    assert!(build_poc_lp_desktop_name("", "desktop").is_err());
    assert!(build_poc_lp_desktop_name("WinSta0\\other", "desktop").is_err());
    assert!(build_poc_lp_desktop_name("WinSta0", "").is_err());
    assert!(build_poc_lp_desktop_name("WinSta0", "nested\\desktop").is_err());
    assert!(build_poc_lp_desktop_name("WinSta0", "desk\0top").is_err());
}

#[test]
fn test_poc_child_exit_status_reports_decimal_hex_and_known_symbol() {
    assert_eq!(format_poc_child_exit_status(0), "decimal=0 hex=0x00000000");
    assert_eq!(
        format_poc_child_exit_status(STATUS_DLL_INIT_FAILED),
        "decimal=3221225794 hex=0xC0000142 status=STATUS_DLL_INIT_FAILED"
    );
    assert_eq!(
        format_poc_child_exit_status(0xDEAD_BEEF),
        "decimal=3735928559 hex=0xDEADBEEF"
    );
}

#[test]
fn test_poc_desktop_permissions_least_privilege_pure() {
    // Assert least privilege rights and exact trustee-to-mask mappings conforming to Microsoft API contracts
    assert_eq!(DESKTOP_CHILD_RIGHTS, 0x00C3);
    assert_eq!(DESKTOP_CONTROLLER_RIGHTS, 0x000600C3);
    assert_eq!(DESKTOP_SYSTEM_RIGHTS, 0x00020041);

    const DESKTOP_READOBJECTS: u32 = 0x0001;
    const DESKTOP_CREATEWINDOW: u32 = 0x0002;
    const DESKTOP_ENUMERATE: u32 = 0x0040;
    const DESKTOP_WRITEOBJECTS: u32 = 0x0080;
    const READ_CONTROL: u32 = 0x00020000;
    const WRITE_DAC: u32 = 0x00040000;

    assert_eq!(
        DESKTOP_CHILD_RIGHTS,
        DESKTOP_READOBJECTS | DESKTOP_CREATEWINDOW | DESKTOP_ENUMERATE | DESKTOP_WRITEOBJECTS
    );

    assert_eq!(
        DESKTOP_CONTROLLER_RIGHTS,
        DESKTOP_CHILD_RIGHTS | READ_CONTROL | WRITE_DAC
    );

    assert_eq!(
        DESKTOP_SYSTEM_RIGHTS,
        DESKTOP_READOBJECTS | DESKTOP_ENUMERATE | READ_CONTROL
    );

    // Microsoft CreateDesktopExW / CreateDesktop requirement:
    // When requesting READ_CONTROL or WRITE_DAC, DESKTOP_READOBJECTS and DESKTOP_WRITEOBJECTS MUST be specified
    assert_ne!(
        DESKTOP_CONTROLLER_RIGHTS & DESKTOP_READOBJECTS,
        0,
        "controller desired access must include DESKTOP_READOBJECTS per CreateDesktop contract"
    );
    assert_ne!(
        DESKTOP_CONTROLLER_RIGHTS & DESKTOP_WRITEOBJECTS,
        0,
        "controller desired access must include DESKTOP_WRITEOBJECTS per CreateDesktop contract"
    );

    // Child must not possess administrative/DACL mutation rights
    assert_eq!(
        DESKTOP_CHILD_RIGHTS & WRITE_DAC,
        0,
        "child rights must not contain WRITE_DAC"
    );
    assert_eq!(
        DESKTOP_CHILD_RIGHTS & READ_CONTROL,
        0,
        "child rights must not contain READ_CONTROL"
    );

    const DESKTOP_ALL: u32 = 0x01FF;
    const GENERIC_ALL: u32 = 0x10000000;

    assert_ne!(
        DESKTOP_CHILD_RIGHTS & DESKTOP_ALL,
        DESKTOP_ALL,
        "child rights must not be DESKTOP_ALL"
    );
    assert_ne!(
        DESKTOP_CONTROLLER_RIGHTS & GENERIC_ALL,
        GENERIC_ALL,
        "controller rights must not be GENERIC_ALL"
    );
    assert_ne!(
        DESKTOP_SYSTEM_RIGHTS & DESKTOP_ALL,
        DESKTOP_ALL,
        "system rights must not be DESKTOP_ALL"
    );
}

#[test]
fn test_poc_window_station_child_rights_least_privilege_pure() {
    const WINSTA_ENUMDESKTOPS: u32 = 0x0001;
    const WINSTA_READATTRIBUTES: u32 = 0x0002;
    const WINSTA_ACCESSCLIPBOARD: u32 = 0x0004;
    const WINSTA_ACCESSGLOBALATOMS: u32 = 0x0020;
    const WINSTA_ENUMERATE: u32 = 0x0100;
    const WINSTA_CREATEDESKTOP: u32 = 0x0008;
    const WINSTA_WRITEATTRIBUTES: u32 = 0x0010;
    const WINSTA_READSCREEN: u32 = 0x0200;
    const WINSTA_EXITWINDOWS: u32 = 0x0040;

    let expected = WINSTA_ENUMDESKTOPS
        | WINSTA_READATTRIBUTES
        | WINSTA_ACCESSCLIPBOARD
        | WINSTA_ACCESSGLOBALATOMS
        | WINSTA_ENUMERATE;
    assert_eq!(poc_window_station_child_rights(), expected);
    assert_eq!(poc_window_station_child_rights(), 0x0127);
    assert_eq!(
        poc_window_station_child_rights()
            & (WINSTA_CREATEDESKTOP
                | WINSTA_WRITEATTRIBUTES
                | WINSTA_READSCREEN
                | WINSTA_EXITWINDOWS),
        0,
        "child must not receive station administration, screen capture, or logoff rights"
    );
}

#[test]
fn test_poc_environment_block_sorting_and_validation_pure() {
    // 1. Valid unordered pairs are sorted case-insensitively
    let pairs = vec![
        ("zebra".to_string(), "1".to_string()),
        ("ALPHA".to_string(), "2".to_string()),
        ("beta".to_string(), "3".to_string()),
    ];
    let block = create_unicode_environment_block(&pairs).expect("valid env block");

    let mut entries = Vec::new();
    let mut current = Vec::new();
    for &ch in &block {
        if ch == 0 {
            if current.is_empty() {
                break;
            }
            entries.push(String::from_utf16_lossy(&current));
            current.clear();
        } else {
            current.push(ch);
        }
    }
    assert_eq!(entries, vec!["ALPHA=2", "beta=3", "zebra=1"]);
    assert_eq!(block[block.len() - 1], 0);
    assert_eq!(block[block.len() - 2], 0);

    // 2. Duplicate case-insensitive keys fail
    let dup_pairs = vec![
        ("VAR_A".to_string(), "1".to_string()),
        ("var_a".to_string(), "2".to_string()),
    ];
    assert!(create_unicode_environment_block(&dup_pairs).is_err());

    // 3. Non-ASCII keys fail
    let non_ascii = vec![("VAR_テスト".to_string(), "1".to_string())];
    assert!(create_unicode_environment_block(&non_ascii).is_err());

    // 4. Malformed keys fail
    assert!(create_unicode_environment_block(&[("".to_string(), "val".to_string())]).is_err());
    assert!(
        create_unicode_environment_block(&[("KEY=VAL".to_string(), "val".to_string())]).is_err()
    );
    assert!(create_unicode_environment_block(&[("KEY\0".to_string(), "val".to_string())]).is_err());
}

#[test]
fn test_poc_typed_denial_classification_pure() {
    // Expected AccessDenied OS error 5 -> DeniedAccessDenied
    let access_denied_err: std::io::Result<()> = Err(std::io::Error::from_raw_os_error(5));
    let outcome = OperationOutcome::from_io_result(&access_denied_err);
    assert_eq!(outcome, OperationOutcome::DeniedAccessDenied);
    assert!(outcome.is_denied());

    // Generic PermissionDenied WITHOUT raw OS error 5 must NOT count as DeniedAccessDenied!
    let perm_err_no_os: std::io::Result<()> = Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "generic permission error",
    ));
    let outcome = OperationOutcome::from_io_result(&perm_err_no_os);
    assert_eq!(outcome, OperationOutcome::FailedOther(-1));
    assert!(
        !outcome.is_denied(),
        "synthetic PermissionDenied without OS code must fail closed"
    );

    // NotFound (error 2) -> FailedOther(2), NOT Denied
    let not_found_err: std::io::Result<()> = Err(std::io::Error::from_raw_os_error(2));
    let outcome = OperationOutcome::from_io_result(&not_found_err);
    assert_eq!(outcome, OperationOutcome::FailedOther(2));
    assert!(!outcome.is_denied());

    // Sharing violation (error 32) -> FailedOther(32), NOT Denied
    let sharing_err: std::io::Result<()> = Err(std::io::Error::from_raw_os_error(32));
    let outcome = OperationOutcome::from_io_result(&sharing_err);
    assert_eq!(outcome, OperationOutcome::FailedOther(32));
    assert!(!outcome.is_denied());

    // Ok -> Allowed
    let ok_res: std::io::Result<()> = Ok(());
    assert_eq!(
        OperationOutcome::from_io_result(&ok_res),
        OperationOutcome::Allowed
    );
    assert!(!OperationOutcome::from_io_result(&ok_res).is_denied());
}

#[test]
fn test_poc_evaluator_matrix_pure() {
    let mut ev = ChildProbeEvidence::default();
    assert!(!ev.is_all_passed());

    ev.child_execution_verified = true;
    ev.controller_vm_write = OperationOutcome::DeniedAccessDenied;
    ev.controller_terminate = OperationOutcome::DeniedAccessDenied;
    ev.controller_dup_handle = OperationOutcome::DeniedAccessDenied;
    ev.controller_query_info = OperationOutcome::Allowed;
    ev.controller_token_duplicate = OperationOutcome::DeniedAccessDenied;

    ev.candidate_create_ok = true;
    ev.candidate_append_ok = true;
    ev.candidate_rename_ok = true;
    ev.candidate_delete_ok = true;
    ev.scratch_create_ok = true;
    ev.scratch_delete_ok = true;

    ev.primary_create = OperationOutcome::DeniedAccessDenied;
    ev.primary_append = OperationOutcome::DeniedAccessDenied;
    ev.primary_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.primary_delete = OperationOutcome::DeniedAccessDenied;
    ev.primary_rename = OperationOutcome::DeniedAccessDenied;
    ev.primary_mkdir = OperationOutcome::DeniedAccessDenied;

    ev.engine_create = OperationOutcome::DeniedAccessDenied;
    ev.engine_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.sentinels_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.outside_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.traversal_write = OperationOutcome::DeniedAccessDenied;

    assert!(ev.is_all_passed(), "all required assertions passed");

    // Toggle each required field off and assert failure
    let mut ev_bad = ev.clone();
    ev_bad.child_execution_verified = false;
    assert!(!ev_bad.is_all_passed());

    let mut ev_bad = ev.clone();
    ev_bad.controller_vm_write = OperationOutcome::Allowed;
    assert!(!ev_bad.is_all_passed());

    let mut ev_bad = ev.clone();
    ev_bad.controller_token_duplicate = OperationOutcome::NotAttempted; // unattempted must fail
    assert!(!ev_bad.is_all_passed());

    let mut ev_bad = ev.clone();
    ev_bad.primary_overwrite = OperationOutcome::FailedOther(2); // NotFound must not pass
    assert!(!ev_bad.is_all_passed());

    let mut ev_bad = ev.clone();
    ev_bad.traversal_write = OperationOutcome::FailedOther(32); // sharing violation must not pass
    assert!(!ev_bad.is_all_passed());
}

#[test]
fn test_poc_token_evidence_validation_pure() {
    let ev = PocTokenEvidence {
        observed_user_sid: "S-1-5-21-12345-67890-1000".to_string(),
        expected_user_sid: "S-1-5-21-12345-67890-1000".to_string(),
        observed_restricted_sids: vec!["S-1-9-1-2-3-4".to_string(), "S-1-5-32-545".to_string()],
        expected_restriction_sid: "S-1-9-1-2-3-4".to_string(),
        privilege_count: 5,
        enabled_privilege_count: 0,
        enabled_privilege_luids: Vec::new(),
        integrity_level_rid: SECURITY_MANDATORY_MEDIUM_RID, // 0x2000
        is_token_restricted_query: true,
    };
    assert!(ev.validate_evidence().is_ok());

    // Low integrity (0x1000) is also valid under policy
    let mut ev_low = ev.clone();
    ev_low.integrity_level_rid = SECURITY_MANDATORY_LOW_RID; // 0x1000
    assert!(ev_low.validate_evidence().is_ok());

    // Mismatched user SID
    let mut ev_bad = ev.clone();
    ev_bad.observed_user_sid = "S-1-5-21-99999-99999-1000".to_string();
    assert!(ev_bad.validate_evidence().is_err());

    // Empty or non-canonical user SID
    let mut ev_bad = ev.clone();
    ev_bad.observed_user_sid = "".to_string();
    assert!(ev_bad.validate_evidence().is_err());
    let mut ev_bad = ev.clone();
    ev_bad.observed_user_sid = "invalid-sid-format".to_string();
    assert!(ev_bad.validate_evidence().is_err());
    for malformed in ["S-1-", "S-1-5-", "S-1-5-4294967296", "S-1-05-1", "S-1-5-01"] {
        let mut malformed_ev = ev.clone();
        malformed_ev.observed_user_sid = malformed.to_string();
        malformed_ev.expected_user_sid = malformed.to_string();
        assert!(
            malformed_ev.validate_evidence().is_err(),
            "accepted {malformed}"
        );
    }

    // The documented SeChangeNotifyPrivilege exception is accepted.
    let mut ev_change_notify = ev.clone();
    ev_change_notify.enabled_privilege_count = 1;
    ev_change_notify
        .enabled_privilege_luids
        .push(SE_CHANGE_NOTIFY_PRIVILEGE_LUID);
    assert!(ev_change_notify.validate_evidence().is_ok());

    // Any other enabled privilege must fail closed.
    let mut ev_bad = ev.clone();
    ev_bad.enabled_privilege_count = 1;
    ev_bad.enabled_privilege_luids.push(PocPrivilegeLuid {
        low_part: 2,
        high_part: 0,
    });
    assert!(ev_bad.validate_evidence().is_err());

    // Count/list inconsistency must fail closed.
    let mut ev_bad = ev_change_notify.clone();
    ev_bad.enabled_privilege_count = 2;
    assert!(ev_bad.validate_evidence().is_err());

    // Zero privileges queried (indicates unqueried or malformed query)
    let mut ev_bad = ev.clone();
    ev_bad.privilege_count = 0;
    assert!(ev_bad.validate_evidence().is_err());

    // Missing restriction SID
    let mut ev_bad = ev.clone();
    ev_bad.observed_restricted_sids = vec!["S-1-5-32-545".to_string()];
    assert!(ev_bad.validate_evidence().is_err());

    // IsTokenRestricted false
    let mut ev_bad = ev.clone();
    ev_bad.is_token_restricted_query = false;
    assert!(ev_bad.validate_evidence().is_err());

    // Integrity level outside policy: Untrusted (0x0000) or High (0x3000)
    let mut ev_bad = ev.clone();
    ev_bad.integrity_level_rid = 0x0000;
    assert!(ev_bad.validate_evidence().is_err());
    let mut ev_bad = ev.clone();
    ev_bad.integrity_level_rid = 0x3000;
    assert!(ev_bad.validate_evidence().is_err());
}

#[test]
fn test_poc_token_open_prerequisite_not_attempted_regression_pure() {
    // Regression test for §151.1.4:
    // If prerequisite OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) fails,
    // OpenProcessToken was never called and must be recorded as NotAttempted,
    // NEVER as DeniedAccessDenied.
    let mut ev = ChildProbeEvidence::default();
    ev.child_execution_verified = true;
    ev.controller_vm_write = OperationOutcome::DeniedAccessDenied;
    ev.controller_terminate = OperationOutcome::DeniedAccessDenied;
    ev.controller_dup_handle = OperationOutcome::DeniedAccessDenied;
    ev.controller_query_info = OperationOutcome::Allowed;
    ev.candidate_create_ok = true;
    ev.candidate_append_ok = true;
    ev.candidate_rename_ok = true;
    ev.candidate_delete_ok = true;
    ev.scratch_create_ok = true;
    ev.scratch_delete_ok = true;
    ev.primary_create = OperationOutcome::DeniedAccessDenied;
    ev.primary_append = OperationOutcome::DeniedAccessDenied;
    ev.primary_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.primary_delete = OperationOutcome::DeniedAccessDenied;
    ev.primary_rename = OperationOutcome::DeniedAccessDenied;
    ev.primary_mkdir = OperationOutcome::DeniedAccessDenied;
    ev.engine_create = OperationOutcome::DeniedAccessDenied;
    ev.engine_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.sentinels_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.outside_overwrite = OperationOutcome::DeniedAccessDenied;
    ev.traversal_write = OperationOutcome::DeniedAccessDenied;

    // Prerequisite failure sets NotAttempted via producer mapping
    ev.controller_token_duplicate = map_process_and_token_open_outcome(Err(5), None);

    assert!(
        !ev.is_all_passed(),
        "unattempted OpenProcessToken must fail is_all_passed and never be accepted as denial proof"
    );

    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), Some(Err(5))),
        OperationOutcome::DeniedAccessDenied,
        "only an attempted OpenProcessToken raw error 5 is denial evidence"
    );
    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), Some(Err(87))),
        OperationOutcome::FailedOther(87)
    );
    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), Some(Ok(()))),
        OperationOutcome::Allowed
    );
}

#[test]
fn test_poc_token_open_producer_mapping_pure() {
    // Enforces §153.2 requirement 5:
    // Test both prerequisite denial and actual OpenProcessToken denial independently.

    // 1. Failed OpenProcess prerequisite with ERROR_ACCESS_DENIED (5) -> NotAttempted
    assert_eq!(
        map_process_and_token_open_outcome(Err(5), None),
        OperationOutcome::NotAttempted
    );

    // 2. Failed OpenProcess prerequisite with other error (e.g. 2, 87) -> NotAttempted
    assert_eq!(
        map_process_and_token_open_outcome(Err(2), None),
        OperationOutcome::NotAttempted
    );
    assert_eq!(
        map_process_and_token_open_outcome(Err(87), None),
        OperationOutcome::NotAttempted
    );

    // 3. OpenProcess failure even if token result is populated -> NotAttempted
    assert_eq!(
        map_process_and_token_open_outcome(Err(5), Some(Err(5))),
        OperationOutcome::NotAttempted
    );

    // 4. OpenProcess succeeded, OpenProcessToken was not called -> NotAttempted
    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), None),
        OperationOutcome::NotAttempted
    );

    // 5. OpenProcess succeeded, OpenProcessToken returned ERROR_ACCESS_DENIED (5) -> DeniedAccessDenied
    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), Some(Err(5))),
        OperationOutcome::DeniedAccessDenied
    );

    // 6. OpenProcess succeeded, OpenProcessToken returned non-5 error (e.g. 1314) -> FailedOther
    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), Some(Err(1314))),
        OperationOutcome::FailedOther(1314)
    );

    // 7. OpenProcess succeeded, OpenProcessToken succeeded -> Allowed
    assert_eq!(
        map_process_and_token_open_outcome(Ok(()), Some(Ok(()))),
        OperationOutcome::Allowed
    );
}

#[test]
fn test_poc_token_binary_parser_rejects_malformed_and_truncated_sid_pure() {
    let mut sid = vec![1, 2, 0, 0, 0, 0, 0, 5];
    sid.extend_from_slice(&21u32.to_le_bytes());
    sid.extend_from_slice(&100u32.to_le_bytes());
    assert_eq!(sid_bytes_to_canonical(&sid).unwrap().0, "S-1-5-21-100");
    assert!(sid_bytes_to_canonical(&sid[..11]).is_err());
    let mut bad_revision = sid.clone();
    bad_revision[0] = 2;
    assert!(sid_bytes_to_canonical(&bad_revision).is_err());
    let mut excessive_count = sid.clone();
    excessive_count[1] = 16;
    assert!(sid_bytes_to_canonical(&excessive_count).is_err());
    assert!(checked_array_len(8, usize::MAX, 16, 128).is_err());
    assert!(checked_array_len(8, 2, 16, 39).is_err());
    assert!(checked_sid_pointer_region(&sid, 0x1000, 0x0FFF).is_err());
    assert!(checked_sid_pointer_region(&sid, 0x1000, 0x100C).is_err());
}

#[test]
fn test_poc_token_information_parsers_validate_layout_and_pointer_ranges_pure() {
    fn write_usize(bytes: &mut [u8], offset: usize, value: usize) {
        let raw = value.to_ne_bytes();
        bytes[offset..offset + raw.len()].copy_from_slice(&raw);
    }
    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    fn sid(authority: u8, rid: u32) -> Vec<u8> {
        let mut bytes = vec![1, 1, 0, 0, 0, 0, 0, authority];
        bytes.extend_from_slice(&rid.to_le_bytes());
        bytes
    }

    assert!(unsafe { validate_token_user_buffer(null(), 8, 8) }.is_err());
    assert!(unsafe { validate_token_groups_buffer(null(), 8, 8) }.is_err());
    assert!(unsafe { validate_token_privileges_buffer(null(), 8, 8) }.is_err());
    assert!(unsafe { validate_token_integrity_buffer(null(), 8, 8) }.is_err());

    let user_header = std::mem::size_of::<TOKEN_USER>();
    let mut user = vec![0; user_header + 12];
    let user_sid_offset = user_header;
    let user_base = user.as_ptr() as usize;
    write_usize(
        &mut user,
        std::mem::offset_of!(TOKEN_USER, User) + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid),
        user_base + user_sid_offset,
    );
    user[user_sid_offset..].copy_from_slice(&sid(5, 1000));
    assert_eq!(
        unsafe { validate_token_user_buffer(user.as_ptr(), user.len() as u32, user.len()) }
            .unwrap(),
        "S-1-5-1000"
    );
    assert!(unsafe {
        validate_token_user_buffer(user.as_ptr(), (user_header - 1) as u32, user.len())
    }
    .is_err());

    let groups_offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    let group_entry_size = std::mem::size_of::<SID_AND_ATTRIBUTES>();
    let groups_sid_offset = groups_offset + group_entry_size;
    let mut groups = vec![0; groups_sid_offset + 12];
    write_u32(&mut groups, 0, 1);
    let groups_base = groups.as_ptr() as usize;
    write_usize(
        &mut groups,
        groups_offset + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid),
        groups_base + groups_sid_offset,
    );
    groups[groups_sid_offset..].copy_from_slice(&sid(9, 123));
    assert_eq!(
        unsafe { validate_token_groups_buffer(groups.as_ptr(), groups.len() as u32, groups.len()) }
            .unwrap(),
        vec!["S-1-9-123"]
    );
    assert!(unsafe {
        validate_token_groups_buffer(
            groups.as_ptr(),
            (groups_sid_offset - 1) as u32,
            groups.len(),
        )
    }
    .is_err());
    write_usize(
        &mut groups,
        groups_offset + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid),
        groups_base + groups_sid_offset + 13,
    );
    assert!(unsafe {
        validate_token_groups_buffer(groups.as_ptr(), groups.len() as u32, groups.len())
    }
    .is_err());

    let privileges_offset = std::mem::offset_of!(TOKEN_PRIVILEGES, Privileges);
    let privilege_entry_size =
        std::mem::size_of::<windows_sys::Win32::Security::LUID_AND_ATTRIBUTES>();
    let mut privileges = vec![0; privileges_offset + privilege_entry_size];
    write_u32(&mut privileges, 0, 1);
    let luid_offset = std::mem::offset_of!(windows_sys::Win32::Security::LUID_AND_ATTRIBUTES, Luid);
    write_u32(
        &mut privileges,
        privileges_offset
            + luid_offset
            + std::mem::offset_of!(windows_sys::Win32::Foundation::LUID, LowPart),
        SE_CHANGE_NOTIFY_PRIVILEGE_LUID.low_part,
    );
    write_u32(
        &mut privileges,
        privileges_offset
            + std::mem::offset_of!(
                windows_sys::Win32::Security::LUID_AND_ATTRIBUTES,
                Attributes
            ),
        0x2,
    );
    assert_eq!(
        unsafe {
            validate_token_privileges_buffer(
                privileges.as_ptr(),
                privileges.len() as u32,
                privileges.len(),
            )
        }
        .unwrap(),
        (1, vec![SE_CHANGE_NOTIFY_PRIVILEGE_LUID])
    );
    assert!(unsafe {
        validate_token_privileges_buffer(
            privileges.as_ptr(),
            (privileges.len() - 1) as u32,
            privileges.len(),
        )
    }
    .is_err());

    let integrity_header = std::mem::size_of::<TOKEN_MANDATORY_LABEL>();
    let mut integrity = vec![0; integrity_header + 12];
    let integrity_sid_offset = integrity_header;
    let integrity_base = integrity.as_ptr() as usize;
    write_usize(
        &mut integrity,
        std::mem::offset_of!(TOKEN_MANDATORY_LABEL, Label)
            + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid),
        integrity_base + integrity_sid_offset,
    );
    integrity[integrity_sid_offset..].copy_from_slice(&sid(16, SECURITY_MANDATORY_MEDIUM_RID));
    assert_eq!(
        unsafe {
            validate_token_integrity_buffer(
                integrity.as_ptr(),
                integrity.len() as u32,
                integrity.len(),
            )
        }
        .unwrap(),
        ("S-1-16-8192".to_string(), SECURITY_MANDATORY_MEDIUM_RID)
    );
    integrity[integrity_sid_offset + 7] = 5;
    assert!(unsafe {
        validate_token_integrity_buffer(integrity.as_ptr(), integrity.len() as u32, integrity.len())
    }
    .is_err());
}

#[test]
fn test_poc_canary_comparator_matrix_pure() {
    let baseline = PocCanaryBaseline {
        file_name: "test_canary.rs".to_string(),
        content_hash: "expected_hash_value_1234".to_string(),
        file_length: 100,
        is_file: true,
        is_dir: false,
        is_symlink: false,
        file_attributes: 0x20, // FILE_ATTRIBUTE_ARCHIVE
    };

    let matching_obs = PocObservedMetadata {
        file_length: 100,
        is_file: true,
        is_dir: false,
        is_symlink: false,
        file_attributes: 0x20,
        content_hash: Some("expected_hash_value_1234".to_string()),
    };
    assert!(baseline.compare_observed(&matching_obs).is_ok());

    // 1. Length mismatch
    let mut obs = matching_obs.clone();
    obs.file_length = 101;
    assert!(baseline.compare_observed(&obs).is_err());

    // 2. is_file mismatch
    let mut obs = matching_obs.clone();
    obs.is_file = false;
    assert!(baseline.compare_observed(&obs).is_err());

    // 3. is_dir mismatch
    let mut obs = matching_obs.clone();
    obs.is_dir = true;
    assert!(baseline.compare_observed(&obs).is_err());

    // 4. is_symlink mismatch
    let mut obs = matching_obs.clone();
    obs.is_symlink = true;
    assert!(baseline.compare_observed(&obs).is_err());

    // 5. file_attributes mismatch
    let mut obs = matching_obs.clone();
    obs.file_attributes = 0x400; // Reparse point attribute
    assert!(baseline.compare_observed(&obs).is_err());

    // 6. Content hash mismatch
    let mut obs = matching_obs.clone();
    obs.content_hash = Some("deviated_content_hash_5678".to_string());
    assert!(baseline.compare_observed(&obs).is_err());

    // 7. Missing content hash
    let mut obs = matching_obs.clone();
    obs.content_hash = None;
    assert!(baseline.compare_observed(&obs).is_err());
}

#[test]
fn test_poc_path_component_integrity_matrix_pure() {
    let test_path = Path::new("C:\\fake\\root\\parent\\child\\leaf.txt");

    // Case 1: All components exist and are clean -> Ok
    let res = evaluate_path_components_integrity(test_path, |_p| {
        Ok(PocFileMetadata {
            is_symlink: false,
            is_reparse_point: false,
        })
    });
    assert!(res.is_ok(), "all present components must pass");

    // Case 2: Contiguous absent suffix (leaf.txt is NotFound) -> Ok
    let res = evaluate_path_components_integrity(test_path, |p| {
        if p.ends_with("leaf.txt") {
            Err(std::io::Error::from_raw_os_error(2)) // NotFound
        } else {
            Ok(PocFileMetadata {
                is_symlink: false,
                is_reparse_point: false,
            })
        }
    });
    assert!(res.is_ok(), "contiguous absent leaf must pass");

    // Case 3: Contiguous absent suffix (child and leaf.txt are NotFound) -> Ok
    let res = evaluate_path_components_integrity(test_path, |p| {
        if p.ends_with("child") || p.ends_with("leaf.txt") {
            Err(std::io::Error::from_raw_os_error(2)) // NotFound
        } else {
            Ok(PocFileMetadata {
                is_symlink: false,
                is_reparse_point: false,
            })
        }
    });
    assert!(res.is_ok(), "contiguous absent suffix must pass");

    // Case 4: Gap / present component after absent ancestor -> Error
    // parent is NotFound, but child is Present
    let res = evaluate_path_components_integrity(test_path, |p| {
        if p.ends_with("parent") {
            Err(std::io::Error::from_raw_os_error(2)) // NotFound
        } else {
            Ok(PocFileMetadata {
                is_symlink: false,
                is_reparse_point: false,
            })
        }
    });
    assert!(
        res.is_err(),
        "present component after absent ancestor must fail closed"
    );
    let err = res.unwrap_err();
    assert!(
        err.contains("path integrity violation"),
        "error must cite path integrity violation: {err}"
    );

    // Case 5: Non-NotFound error at root -> fails closed
    let res = evaluate_path_components_integrity(test_path, |p| {
        if p == Path::new("C:\\") {
            Err(std::io::Error::from_raw_os_error(5)) // AccessDenied
        } else {
            Ok(PocFileMetadata {
                is_symlink: false,
                is_reparse_point: false,
            })
        }
    });
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("metadata inspection failed"));

    // Every component position, including the leaf, fails closed for non-NotFound errors.
    let components: Vec<PathBuf> = test_path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect();
    for failed_component in &components {
        let res = evaluate_path_components_integrity(test_path, |p| {
            if p == failed_component {
                Err(std::io::Error::from_raw_os_error(31))
            } else {
                Ok(PocFileMetadata {
                    is_symlink: false,
                    is_reparse_point: false,
                })
            }
        });
        assert!(
            res.is_err(),
            "component error was accepted: {}",
            failed_component.display()
        );
    }

    // Case 6: Non-NotFound error at intermediate component -> fails closed
    let res = evaluate_path_components_integrity(test_path, |p| {
        if p.ends_with("parent") {
            Err(std::io::Error::from_raw_os_error(31)) // GenFailure
        } else {
            Ok(PocFileMetadata {
                is_symlink: false,
                is_reparse_point: false,
            })
        }
    });
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("metadata inspection failed"));

    // Case 7: Symlink component rejected
    let res = evaluate_path_components_integrity(test_path, |p| {
        Ok(PocFileMetadata {
            is_symlink: p.ends_with("parent"),
            is_reparse_point: false,
        })
    });
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must not be a symlink"));

    // Case 8: Reparse point component rejected
    let res = evaluate_path_components_integrity(test_path, |p| {
        Ok(PocFileMetadata {
            is_symlink: false,
            is_reparse_point: p.ends_with("child"),
        })
    });
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must not be a reparse point"));
}

#[test]
fn test_poc_path_component_integrity_table_driven_pure() {
    // Enforces §153.2 requirement 5 / §153.1 finding 6:
    // Comprehensive table-driven case covering every path component index.
    let test_path = Path::new("C:\\fake\\root\\parent\\child\\leaf.txt");
    let mut ancestors: Vec<&Path> = test_path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    ancestors.reverse();
    // ancestors order:
    // 0: C:\
    // 1: C:\fake
    // 2: C:\fake\root
    // 3: C:\fake\root\parent
    // 4: C:\fake\root\parent\child
    // 5: C:\fake\root\parent\child\leaf.txt
    let n = ancestors.len();
    assert_eq!(n, 6);

    // Sub-matrix A: For every component index i, inject non-NotFound error (e.g. 5, 31) -> fails closed
    for (i, target) in ancestors.iter().enumerate() {
        let res = evaluate_path_components_integrity(test_path, |p| {
            if p == *target {
                Err(std::io::Error::from_raw_os_error(31))
            } else {
                Ok(PocFileMetadata {
                    is_symlink: false,
                    is_reparse_point: false,
                })
            }
        });
        assert!(
            res.is_err(),
            "injected non-NotFound error at index {i} ('{}') must fail closed",
            target.display()
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("metadata inspection failed"),
            "error for index {i} must mention metadata inspection failure: {err}"
        );
    }

    // Sub-matrix B: For every component index i, inject symlink -> fails closed
    for (i, target) in ancestors.iter().enumerate() {
        let res = evaluate_path_components_integrity(test_path, |p| {
            Ok(PocFileMetadata {
                is_symlink: p == *target,
                is_reparse_point: false,
            })
        });
        assert!(
            res.is_err(),
            "injected symlink at index {i} ('{}') must be rejected",
            target.display()
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("must not be a symlink"),
            "error for index {i} must cite symlink rejection: {err}"
        );
    }

    // Sub-matrix C: For every component index i, inject reparse point -> fails closed
    for (i, target) in ancestors.iter().enumerate() {
        let res = evaluate_path_components_integrity(test_path, |p| {
            Ok(PocFileMetadata {
                is_symlink: false,
                is_reparse_point: p == *target,
            })
        });
        assert!(
            res.is_err(),
            "injected reparse point at index {i} ('{}') must be rejected",
            target.display()
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("must not be a reparse point"),
            "error for index {i} must cite reparse point rejection: {err}"
        );
    }

    // Sub-matrix D: For every suffix start index i in 1..n, components from index i to leaf are NotFound (contiguous absent suffix) -> Ok
    for suffix_start in 1..n {
        let res = evaluate_path_components_integrity(test_path, |p| {
            let idx = ancestors.iter().position(|a| *a == p).unwrap();
            if idx >= suffix_start {
                Err(std::io::Error::from_raw_os_error(2))
            } else {
                Ok(PocFileMetadata {
                    is_symlink: false,
                    is_reparse_point: false,
                })
            }
        });
        assert!(
            res.is_ok(),
            "contiguous absent suffix starting at index {suffix_start} ('{}') must pass",
            ancestors[suffix_start].display()
        );
    }

    // Sub-matrix E: For every gap position (component at index i is NotFound, but component at index i+1 is present) -> fails closed
    for gap_idx in 0..n - 1 {
        let res = evaluate_path_components_integrity(test_path, |p| {
            let idx = ancestors.iter().position(|a| *a == p).unwrap();
            if idx == gap_idx {
                Err(std::io::Error::from_raw_os_error(2))
            } else {
                Ok(PocFileMetadata {
                    is_symlink: false,
                    is_reparse_point: false,
                })
            }
        });
        assert!(
            res.is_err(),
            "gap where index {gap_idx} is absent but later index is present must fail closed"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("path integrity violation"),
            "error must cite path integrity violation: {err}"
        );
    }
}

#[test]
fn test_poc_post_mutation_failure_handling_pure() {
    let dummy_path = PathBuf::from("C:\\fake\\preserved\\fixture");

    // Case 1: Run error + restoration succeeds -> fixture PRESERVED on uncertainty
    let mut restore_called = false;
    let mut preserve_called = false;
    let mut cleanup_called = false;
    let outcome = handle_post_mutation_exit(
        Some("child test assertion failed".to_string()),
        || {
            restore_called = true;
            Ok(())
        },
        || {
            preserve_called = true;
            dummy_path.clone()
        },
        || {
            cleanup_called = true;
        },
    );
    assert!(
        restore_called,
        "restoration must be attempted on run failure"
    );
    assert!(preserve_called, "fixture must be preserved on run failure");
    assert!(!cleanup_called, "cleanup must not occur on run failure");
    assert!(outcome.restore_attempted);
    assert!(outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);
    assert_eq!(
        outcome.final_state,
        PocPostMutationState::RestoreAttemptedSuccess
    );
    assert_eq!(
        outcome.error_message,
        Some("child test assertion failed".to_string())
    );

    // Case 2: Run error + restoration fails -> fixture PRESERVED
    let mut restore_called = false;
    let mut preserve_called = false;
    let outcome = handle_post_mutation_exit(
        Some("child timeout".to_string()),
        || {
            restore_called = true;
            Err("SetNamedSecurityInfo failed".to_string())
        },
        || {
            preserve_called = true;
            dummy_path.clone()
        },
        || {},
    );
    assert!(restore_called);
    assert!(preserve_called);
    assert!(outcome.restore_attempted);
    assert!(!outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);
    assert_eq!(
        outcome.final_state,
        PocPostMutationState::RestoreAttemptedFailed
    );

    // Case 3: Run succeeds + restoration succeeds -> fixture CLEANED UP
    let mut restore_called = false;
    let mut preserve_called = false;
    let mut cleanup_called = false;
    let outcome = handle_post_mutation_exit(
        None,
        || {
            restore_called = true;
            Ok(())
        },
        || {
            preserve_called = true;
            dummy_path.clone()
        },
        || {
            cleanup_called = true;
        },
    );
    assert!(restore_called);
    assert!(!preserve_called);
    assert!(cleanup_called);
    assert!(outcome.restore_attempted);
    assert!(outcome.restore_succeeded);
    assert!(!outcome.fixture_preserved);
    assert_eq!(outcome.final_state, PocPostMutationState::FixtureCleanedUp);
    assert!(outcome.error_message.is_none());

    // Case 4: Run succeeds + restoration fails -> fixture PRESERVED
    let mut restore_called = false;
    let mut preserve_called = false;
    let mut cleanup_called = false;
    let outcome = handle_post_mutation_exit(
        None,
        || {
            restore_called = true;
            Err("restore DACL mismatch".to_string())
        },
        || {
            preserve_called = true;
            dummy_path.clone()
        },
        || {
            cleanup_called = true;
        },
    );
    assert!(restore_called);
    assert!(preserve_called);
    assert!(!cleanup_called);
    assert!(outcome.restore_attempted);
    assert!(!outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);
    assert_eq!(
        outcome.final_state,
        PocPostMutationState::RestoreAttemptedFailed
    );
    assert!(outcome
        .error_message
        .unwrap()
        .contains("restoration failed"));

    // A panicking restore callback is caught by the same decision seam and preserves.
    let mut preserved_after_panic = false;
    let outcome = handle_post_mutation_exit(
        None,
        || -> Result<(), String> { panic!("injected restore panic") },
        || {
            preserved_after_panic = true;
            dummy_path.clone()
        },
        || panic!("cleanup must not run after restore panic"),
    );
    assert!(!outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);
    assert!(preserved_after_panic);
    assert!(outcome
        .error_message
        .unwrap()
        .contains("restoration panicked"));

    let run_error = run_post_mutation_catching_panic(|| -> Result<(), String> {
        panic!("injected post-mutation panic")
    })
    .expect_err("panic must become a recovery input");
    let mut restore_after_run_panic = false;
    let mut preserve_after_run_panic = false;
    let outcome = handle_post_mutation_exit(
        Some(run_error),
        || {
            restore_after_run_panic = true;
            Ok(())
        },
        || {
            preserve_after_run_panic = true;
            dummy_path.clone()
        },
        || panic!("cleanup must not run after execution panic"),
    );
    assert!(restore_after_run_panic);
    assert!(preserve_after_run_panic);
    assert!(outcome.fixture_preserved);
}

#[test]
fn test_poc_restore_all_targets_attempts_later_targets_after_failure_pure() {
    let targets = vec![PathBuf::from("first"), PathBuf::from("second")];
    let mut attempted = Vec::new();
    let result = restore_all_targets(&targets, |target| {
        attempted.push(target.to_path_buf());
        if target == Path::new("first") {
            Err("injected failure".into())
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert_eq!(attempted, targets, "all mutated targets must be attempted");
}

#[test]
fn test_poc_multi_target_restoration_fault_injection_pure() {
    // Enforces §153.2 requirement 2 / §153.1 finding 2:
    // Multi-target restoration attempts every target even if earlier target fails or panics,
    // collecting per-target attempt/result/verification in a structured report.
    let t0 = PathBuf::from("C:\\fake\\target0");
    let t1 = PathBuf::from("C:\\fake\\target1");
    let t2 = PathBuf::from("C:\\fake\\target2");
    let targets = vec![t0.clone(), t1.clone(), t2.clone()];

    // Case A: First target fails, later targets succeed
    let mut attempted = Vec::new();
    let report = restore_all_targets_recorded(&targets, |target| {
        attempted.push(target.to_path_buf());
        if target == &t0 {
            Err("failed restoring t0".into())
        } else {
            Ok(())
        }
    });
    assert_eq!(attempted.len(), 3, "every target must be attempted");
    assert!(!report.is_all_success());
    assert_eq!(report.records.len(), 3);
    assert!(!report.records[0].restored);
    assert!(!report.records[0].verified);
    assert!(report.records[0].error.is_some());
    assert!(report.records[1].restored && report.records[1].verified);
    assert!(report.records[2].restored && report.records[2].verified);
    assert!(report
        .combined_error()
        .unwrap()
        .contains("failed restoring t0"));

    // Case B: Middle target panics, earlier and later targets succeed
    let report = restore_all_targets_recorded(&targets, |target| {
        if target == &t1 {
            panic!("injected panic on t1");
        } else {
            Ok(())
        }
    });
    assert!(!report.is_all_success());
    assert_eq!(report.records.len(), 3);
    assert!(report.records[0].restored && report.records[0].verified);
    assert!(!report.records[1].restored && !report.records[1].verified);
    assert!(report.records[1]
        .error
        .as_ref()
        .unwrap()
        .contains("restoration panicked"));
    assert!(report.records[2].restored && report.records[2].verified);

    // Case C: All targets succeed
    let report = restore_all_targets_recorded(&targets, |_target| Ok(()));
    assert!(report.is_all_success());
    assert!(report.combined_error().is_none());
    assert_eq!(report.records.len(), 3);
    for rec in &report.records {
        assert!(rec.restored && rec.verified && rec.error.is_none());
    }

    // Case D: All targets fail
    let report = restore_all_targets_recorded(&targets, |target| {
        Err(format!("error on {}", target.display()))
    });
    assert!(!report.is_all_success());
    let combined = report.combined_error().unwrap();
    assert!(combined.contains("target0"));
    assert!(combined.contains("target1"));
    assert!(combined.contains("target2"));
}

#[test]
fn test_poc_decision_seam_and_fixture_panic_safety_pure() {
    // Enforces §153.2 requirements 1 & 3:
    // 1. Concrete PocRecoveryTarget implementation tested directly without RefCell.
    // 2. Panic safety: unwinds preserve fixture and never delete investigation artifacts.
    struct MockTarget {
        restore_calls: usize,
        preserve_calls: usize,
        cleanup_calls: usize,
        should_restore_fail: bool,
        should_restore_panic: bool,
    }

    impl PocRecoveryTarget for MockTarget {
        fn restore_target(&mut self) -> Result<(), String> {
            self.restore_calls += 1;
            if self.should_restore_panic {
                panic!("mock restore panic");
            }
            if self.should_restore_fail {
                Err("mock restore failure".into())
            } else {
                Ok(())
            }
        }
        fn preserve_target(&mut self) -> PathBuf {
            self.preserve_calls += 1;
            PathBuf::from("C:\\fake\\preserved")
        }
        fn cleanup_target(&mut self) {
            self.cleanup_calls += 1;
        }
    }

    // Test 1: Run error + restore success -> preserved, no cleanup
    let mut target = MockTarget {
        restore_calls: 0,
        preserve_calls: 0,
        cleanup_calls: 0,
        should_restore_fail: false,
        should_restore_panic: false,
    };
    let outcome = handle_post_mutation_decision(&mut target, Some("run failed".into()));
    assert_eq!(target.restore_calls, 1);
    assert_eq!(target.preserve_calls, 1);
    assert_eq!(target.cleanup_calls, 0);
    assert!(outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);

    // Test 2: Run error + restore panic -> panic caught, preserved, no cleanup
    let mut target = MockTarget {
        restore_calls: 0,
        preserve_calls: 0,
        cleanup_calls: 0,
        should_restore_fail: false,
        should_restore_panic: true,
    };
    let outcome = handle_post_mutation_decision(&mut target, Some("run failed".into()));
    assert_eq!(target.restore_calls, 1);
    assert_eq!(target.preserve_calls, 1);
    assert_eq!(target.cleanup_calls, 0);
    assert!(!outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);

    // Test 3: Run success + restore panic -> panic caught, preserved, no cleanup
    let mut target = MockTarget {
        restore_calls: 0,
        preserve_calls: 0,
        cleanup_calls: 0,
        should_restore_fail: false,
        should_restore_panic: true,
    };
    let outcome = handle_post_mutation_decision(&mut target, None);
    assert_eq!(target.restore_calls, 1);
    assert_eq!(target.preserve_calls, 1);
    assert_eq!(target.cleanup_calls, 0);
    assert!(!outcome.restore_succeeded);
    assert!(outcome.fixture_preserved);

    // Test 4: Run success + restore success -> cleaned up, not preserved
    let mut target = MockTarget {
        restore_calls: 0,
        preserve_calls: 0,
        cleanup_calls: 0,
        should_restore_fail: false,
        should_restore_panic: false,
    };
    let outcome = handle_post_mutation_decision(&mut target, None);
    assert_eq!(target.restore_calls, 1);
    assert_eq!(target.preserve_calls, 0);
    assert_eq!(target.cleanup_calls, 1);
    assert!(outcome.restore_succeeded);
    assert!(!outcome.fixture_preserved);

    // Test 5: Verify PocFixture Drop preserves directory on panic / recovery_required
    let preserved_root = {
        let mut fixture = PocFixture::new().expect("create fixture");
        let root = fixture.root_path.clone();
        assert!(root.exists(), "fixture root should exist");
        // Simulate a failure or mutation occurring
        fixture.recovery_required = true;
        // Dropping fixture here must disarm TempDir auto-deletion
        drop(fixture);
        root
    };
    assert!(
        preserved_root.exists(),
        "fixture directory must remain on disk when recovery_required was set before drop"
    );
    // Explicit manual cleanup of test preserved dir
    let _ = std::fs::remove_dir_all(&preserved_root);

    // Test 6: Verify PocFixture Drop with mutated_paths preserves directory
    let preserved_root = {
        let mut fixture = PocFixture::new().expect("create fixture");
        let root = fixture.root_path.clone();
        assert!(root.exists());
        fixture.mutated_paths.push(root.join("candidate"));
        // Dropping fixture with mutated_paths must preserve
        drop(fixture);
        root
    };
    assert!(
        preserved_root.exists(),
        "fixture directory must remain on disk when mutated_paths was non-empty before drop"
    );
    let _ = std::fs::remove_dir_all(&preserved_root);
}

#[test]
fn test_poc_token_buffer_structural_validation_pure() {
    // Enforces §153.2 requirement 4:
    // Pure byte-slice parser tests verifying structural validation before unsafe access.
    fn write_usize(bytes: &mut [u8], offset: usize, value: usize) {
        let raw = value.to_ne_bytes();
        bytes[offset..offset + raw.len()].copy_from_slice(&raw);
    }
    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    fn make_valid_sid(authority: u8, subauthority: u32) -> Vec<u8> {
        let mut b = vec![1, 1, 0, 0, 0, 0, 0, authority];
        b.extend_from_slice(&subauthority.to_le_bytes());
        b
    }

    // --- 1. TokenUser Byte Slice Tests ---
    let header_size = std::mem::size_of::<TOKEN_USER>();
    let sid_bytes = make_valid_sid(5, 500); // S-1-5-500
    let total_len = header_size + sid_bytes.len();
    let base = 0x20000;

    // Truncated buffer (< header_size)
    assert!(parse_token_user_from_bytes(&vec![0; header_size - 1], base).is_err());

    // Valid buffer
    let mut user_buf = vec![0u8; total_len];
    let sid_ptr_offset =
        std::mem::offset_of!(TOKEN_USER, User) + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid);
    write_usize(&mut user_buf, sid_ptr_offset, base + header_size);
    user_buf[header_size..].copy_from_slice(&sid_bytes);
    assert_eq!(
        parse_token_user_from_bytes(&user_buf, base).unwrap(),
        "S-1-5-500"
    );

    // SID pointer points before base address
    let mut bad_ptr_buf = user_buf.clone();
    write_usize(&mut bad_ptr_buf, sid_ptr_offset, base - 1);
    assert!(parse_token_user_from_bytes(&bad_ptr_buf, base).is_err());

    // SID pointer overlaps fixed structure header
    let mut overlap_buf = user_buf.clone();
    write_usize(&mut overlap_buf, sid_ptr_offset, base + 2);
    assert!(parse_token_user_from_bytes(&overlap_buf, base).is_err());

    // SID pointer points past end of buffer
    let mut out_of_bounds_buf = user_buf.clone();
    write_usize(&mut out_of_bounds_buf, sid_ptr_offset, base + total_len);
    assert!(parse_token_user_from_bytes(&out_of_bounds_buf, base).is_err());

    // --- 2. TokenGroups Byte Slice Tests ---
    let groups_offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    let entry_size = std::mem::size_of::<SID_AND_ATTRIBUTES>();
    let group_sid = make_valid_sid(9, 12345);
    let group_total = groups_offset + entry_size + group_sid.len();
    let mut group_buf = vec![0u8; group_total];
    write_u32(&mut group_buf, 0, 1); // 1 group
    let entry_sid_offset = groups_offset + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid);
    write_usize(
        &mut group_buf,
        entry_sid_offset,
        base + groups_offset + entry_size,
    );
    group_buf[groups_offset + entry_size..].copy_from_slice(&group_sid);
    let res = parse_token_groups_from_bytes(&group_buf, base).unwrap();
    assert_eq!(res, vec!["S-1-9-12345".to_string()]);

    // Truncated buffer (< 4 bytes)
    assert!(parse_token_groups_from_bytes(&[0; 3], base).is_err());

    // 0 groups rejected
    let mut zero_groups = group_buf.clone();
    write_u32(&mut zero_groups, 0, 0);
    assert!(parse_token_groups_from_bytes(&zero_groups, base).is_err());

    // Count exceeds buffer length
    let mut overflow_groups = group_buf.clone();
    write_u32(&mut overflow_groups, 0, 100);
    assert!(parse_token_groups_from_bytes(&overflow_groups, base).is_err());

    // --- 3. TokenPrivileges Byte Slice Tests ---
    let priv_offset = std::mem::offset_of!(TOKEN_PRIVILEGES, Privileges);
    let priv_entry_size = std::mem::size_of::<windows_sys::Win32::Security::LUID_AND_ATTRIBUTES>();
    let mut priv_buf = vec![0u8; priv_offset + priv_entry_size * 2];
    write_u32(&mut priv_buf, 0, 2); // 2 privileges
    let attr_offset = std::mem::offset_of!(
        windows_sys::Win32::Security::LUID_AND_ATTRIBUTES,
        Attributes
    );
    let luid_offset = std::mem::offset_of!(windows_sys::Win32::Security::LUID_AND_ATTRIBUTES, Luid);
    let low_part_offset = std::mem::offset_of!(windows_sys::Win32::Foundation::LUID, LowPart);
    let high_part_offset = std::mem::offset_of!(windows_sys::Win32::Foundation::LUID, HighPart);
    // Privilege 0: disabled (0)
    write_u32(&mut priv_buf, priv_offset + attr_offset, 0);
    // Privilege 1: enabled (2)
    write_u32(
        &mut priv_buf,
        priv_offset + priv_entry_size + attr_offset,
        2,
    );
    // SeChangeNotifyPrivilege (LUID 23:0), the documented DISABLE_MAX_PRIVILEGE exception.
    write_u32(
        &mut priv_buf,
        priv_offset + priv_entry_size + luid_offset + low_part_offset,
        SE_CHANGE_NOTIFY_PRIVILEGE_LUID.low_part,
    );
    write_u32(
        &mut priv_buf,
        priv_offset + priv_entry_size + luid_offset + high_part_offset,
        SE_CHANGE_NOTIFY_PRIVILEGE_LUID.high_part as u32,
    );
    assert_eq!(
        parse_token_privileges_from_bytes(&priv_buf).unwrap(),
        (2, vec![SE_CHANGE_NOTIFY_PRIVILEGE_LUID])
    );

    // Zero privileges rejected
    let mut zero_priv = priv_buf.clone();
    write_u32(&mut zero_priv, 0, 0);
    assert!(parse_token_privileges_from_bytes(&zero_priv).is_err());

    // Truncated buffer (< 4 bytes)
    assert!(parse_token_privileges_from_bytes(&[0; 2]).is_err());

    // --- 4. TokenIntegrityLevel Byte Slice Tests ---
    let label_header = std::mem::size_of::<TOKEN_MANDATORY_LABEL>();
    let valid_int_sid = make_valid_sid(16, SECURITY_MANDATORY_LOW_RID);
    let mut int_buf = vec![0u8; label_header + valid_int_sid.len()];
    let label_sid_offset = std::mem::offset_of!(TOKEN_MANDATORY_LABEL, Label)
        + std::mem::offset_of!(SID_AND_ATTRIBUTES, Sid);
    write_usize(&mut int_buf, label_sid_offset, base + label_header);
    int_buf[label_header..].copy_from_slice(&valid_int_sid);
    let (sid_str, rid) = parse_token_integrity_from_bytes(&int_buf, base).unwrap();
    assert_eq!(sid_str, "S-1-16-4096");
    assert_eq!(rid, SECURITY_MANDATORY_LOW_RID);

    // Authority != 16 rejected (e.g. authority 5)
    let bad_auth_sid = make_valid_sid(5, SECURITY_MANDATORY_LOW_RID);
    let mut bad_auth_buf = int_buf.clone();
    bad_auth_buf[label_header..].copy_from_slice(&bad_auth_sid);
    assert!(parse_token_integrity_from_bytes(&bad_auth_buf, base).is_err());

    // Subauthority count != 1 rejected (e.g. 2 subauthorities)
    let mut two_subs_sid = vec![1, 2, 0, 0, 0, 0, 0, 16];
    two_subs_sid.extend_from_slice(&1000u32.to_le_bytes());
    two_subs_sid.extend_from_slice(&2000u32.to_le_bytes());
    let mut two_subs_buf = vec![0u8; label_header + two_subs_sid.len()];
    write_usize(&mut two_subs_buf, label_sid_offset, base + label_header);
    two_subs_buf[label_header..].copy_from_slice(&two_subs_sid);
    assert!(parse_token_integrity_from_bytes(&two_subs_buf, base).is_err());
}

// ---------------------------------------------------------------------------
// Native Runtime Tests (Marked #[ignore]; requires separate authorization)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_poc_restriction_sid_allocation_native() {
    let u = uuid::Uuid::new_v4();
    let sid = PocRestrictionSid::new(u).expect("allocate sid");
    assert!(sid.string_representation.starts_with("S-1-9-"));
}

#[test]
#[ignore]
fn test_poc_desktop_creation_and_cleanup_native() {
    let u = uuid::Uuid::new_v4();
    let sid = PocRestrictionSid::new(u).expect("allocate sid");
    let desk = create_poc_isolated_desktop(u, &sid).expect("create desktop");
    assert!(desk.name.starts_with("Poverello-PoC-Desktop-"));
}

#[test]
#[ignore]
fn test_poc_job_object_configuration_native() {
    let u = uuid::Uuid::new_v4();
    let job = PocJobObject::new(u).expect("create job");
    assert!(job.name.starts_with("Poverello-PoC-Job-"));
}

#[test]
#[ignore]
fn test_poc_fixture_setup_and_sentinel_integrity_native() {
    let fixture = PocFixture::new().expect("create fixture");
    assert!(fixture.verify_canaries_unmodified().is_ok());
}

#[test]
#[ignore]
fn test_poc_child_entrypoint() {
    let role = std::env::var("POVERELLO_POC_CHILD_ROLE").unwrap_or_default();
    if role != "1" {
        return;
    }
    run_child_probe_implementation().expect("child probe execution failed");
}

#[test]
#[ignore]
fn test_stage_2_restricted_token_filesystem_poc_native() {
    let opt_in = std::env::var("POVERELLO_STAGE2_POC_RESTRICTED_TOKEN").ok();
    if !evaluate_poc_opt_in(opt_in.as_deref()) {
        panic!("[UNVERIFIED] POVERELLO_STAGE2_POC_RESTRICTED_TOKEN != 1; execution refused without opt-in");
    }

    let helper_path = require_poc_native_startup_helper_path(
        std::env::var_os("POVERELLO_POC_STARTUP_HELPER_EXE")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let target_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let helper_path = validate_poc_native_startup_helper_path(&helper_path, &target_root)
        .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let user32_helper_path = require_poc_user32_startup_helper_path(
        std::env::var_os("POVERELLO_POC_USER32_STARTUP_HELPER_EXE")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let user32_helper_path =
        validate_poc_user32_startup_helper_path(&user32_helper_path, &target_root)
            .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let userenv_helper_path = require_poc_userenv_startup_helper_path(
        std::env::var_os("POVERELLO_POC_USERENV_STARTUP_HELPER_EXE")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let userenv_helper_path =
        validate_poc_userenv_startup_helper_path(&userenv_helper_path, &target_root)
            .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let vcruntime_helper_path = require_poc_vcruntime_startup_helper_path(
        std::env::var_os("POVERELLO_POC_VCRUNTIME_STARTUP_HELPER_EXE")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let vcruntime_helper_path =
        validate_poc_vcruntime_startup_helper_path(&vcruntime_helper_path, &target_root)
            .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let crt_runtime_helper_path = require_poc_crt_runtime_startup_helper_path(
        std::env::var_os("POVERELLO_POC_CRT_RUNTIME_STARTUP_HELPER_EXE")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let crt_runtime_helper_path =
        validate_poc_crt_runtime_startup_helper_path(&crt_runtime_helper_path, &target_root)
            .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let grouped_crt_helper_path = require_poc_grouped_crt_startup_helper_path(
        std::env::var_os("POVERELLO_POC_GROUPED_CRT_STARTUP_HELPER_EXE")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));
    let grouped_crt_helper_path =
        validate_poc_grouped_crt_startup_helper_path(&grouped_crt_helper_path, &target_root)
            .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));

    let cdb_path = resolve_cdb_diagnostic_path(
        std::env::var("POVERELLO_STAGE2_CDB_DIAGNOSTIC")
            .ok()
            .as_deref(),
        std::env::var_os("POVERELLO_STAGE2_CDB_PATH")
            .as_deref()
            .map(Path::new),
    )
    .unwrap_or_else(|error| panic!("[UNVERIFIED] {error}"));

    let uuid = uuid::Uuid::new_v4();
    println!("[POC-RUN] Starting Bounded Restricted-Token PoC run: {uuid}");

    // 1. Derive unique per-run restriction SID
    let restriction_sid = PocRestrictionSid::new(uuid).expect("derive restriction SID");
    println!(
        "[POC-TOKEN] Derived unique restriction SID: {}",
        restriction_sid.string_representation
    );

    // 2. Create disposable fixture
    let mut fixture = PocFixture::new().expect("create disposable fixture");
    println!(
        "[POC-FIXTURE] Created disposable fixture root: {}",
        fixture.root_path.display()
    );

    // Keep the station guard outside the unwind-catching closure so the exact
    // original DACL can be restored even when the probe body returns an error
    // or panics after applying the temporary ACE.
    let mut station_guard: Option<PocWindowStationDaclGuard> = None;
    let mut cdb_report: Option<PendingCdbReport> = cdb_path
        .as_ref()
        .map(|_| PendingCdbReport::new(uuid.to_string()));

    // Centralized post-mutation execution closure ensuring fixture preservation on ANY failure
    let run_res = run_post_mutation_catching_panic(|| -> Result<(), String> {
        fixture
            .apply_restriction_sid_write_grants(&restriction_sid)
            .map_err(|e| format!("apply restriction grants: {e}"))?;

        // 3. Create isolated non-default desktop with documented least-privilege DACL
        let desktop_guard = create_poc_isolated_desktop(uuid, &restriction_sid)
            .map_err(|e| format!("create isolated desktop: {e}"))?;

        // 4. Create Job Object
        let job = PocJobObject::new(uuid).map_err(|e| format!("create job object: {e}"))?;

        // 5. Create restricted token with WRITE_RESTRICTED | DISABLE_MAX_PRIVILEGE
        let (restricted_token, token_evidence) = create_poc_restricted_token(&restriction_sid)
            .map_err(|e| format!("create restricted token: {e}"))?;

        token_evidence
            .validate_evidence()
            .map_err(|e| format!("token evidence validation: {e}"))?;

        // Read-only diagnostic: record the current Window Station and explicit ACEs
        // for the restricting SID. This is intentionally not an effective access
        // check and never modifies the station DACL.
        match unsafe { inspect_window_station_sid_aces(&restriction_sid) } {
            Ok(station) => println!(
                "[POC-WINDOW-STATION] name={:?} dacl_is_null={} restricting_sid_allow_masks={:?} restricting_sid_deny_masks={:?} effective_access=NOT_COMPUTED",
                station.station_name,
                station.dacl_is_null,
                station.allow_masks,
                station.deny_masks
            ),
            Err(error) => println!(
                "[POC-WINDOW-STATION] diagnostic=UNAVAILABLE reason={error}; continuing_without_station_verdict"
            ),
        }

        // Temporarily grant only the run-unique restricting SID the minimum
        // station rights needed by the child. This is restored immediately
        // after the child attempt and before fixture recovery/cleanup.
        station_guard = Some(PocWindowStationDaclGuard::capture_current(
            &restriction_sid,
        )?);
        station_guard
            .as_mut()
            .expect("station guard was just initialized")
            .grant_child_station_access()?;
        println!(
            "[POC-WINDOW-STATION] station_acl_mutation_attempted={} child_rights=0x{:04X} station={:?}",
            station_guard.as_ref().unwrap().mutation_attempted,
            poc_window_station_child_rights(),
            station_guard.as_ref().unwrap().station_name
        );

        // 6. Launch a no-op system process first to distinguish broad startup
        // failures from failures specific to the Rust test executable.
        let controller_pid = unsafe { GetCurrentProcessId() };
        let smoke_environment = poc_child_environment_pairs(&fixture, controller_pid);
        let native_helper_exit = launch_poc_native_startup_helper(
            restricted_token.0,
            &helper_path,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| {
            format!("restricted native startup helper failed before other children: {e}")
        })?;
        println!(
            "[POC-STARTUP-HELPER] executable={} result=EXITED_ZERO exit_code={native_helper_exit}",
            helper_path.display()
        );

        let smoke_exit = launch_poc_system_cmd_smoke_child(
            restricted_token.0,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| format!("restricted startup smoke failed before Rust probe: {e}"))?;
        println!(
            "[POC-STARTUP-SMOKE] executable=System32\\cmd.exe result=EXITED_ZERO exit_code={smoke_exit}"
        );

        // 6a. Compare a USER32-linked native process before the Rust test child.
        let user32_helper_exit = launch_poc_native_startup_helper(
            restricted_token.0,
            &user32_helper_path,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| format!("restricted USER32 startup helper failed before Rust probe: {e}"))?;
        println!(
            "[POC-USER32-STARTUP] executable={} result=EXITED_ZERO exit_code={user32_helper_exit}",
            user32_helper_path.display()
        );

        let userenv_helper_exit = launch_poc_native_startup_helper(
            restricted_token.0,
            &userenv_helper_path,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| format!("restricted USERENV startup helper failed before Rust probe: {e}"))?;
        println!(
            "[POC-USERENV-STARTUP] executable={} result=EXITED_ZERO exit_code={userenv_helper_exit}",
            userenv_helper_path.display()
        );

        let vcruntime_helper_exit = launch_poc_native_startup_helper(
            restricted_token.0,
            &vcruntime_helper_path,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| {
            format!("restricted VCRUNTIME startup helper failed before Rust probe: {e}")
        })?;
        println!(
            "[POC-VCRUNTIME-STARTUP] executable={} result=EXITED_ZERO exit_code={vcruntime_helper_exit}",
            vcruntime_helper_path.display()
        );

        let crt_runtime_helper_exit = launch_poc_native_startup_helper(
            restricted_token.0,
            &crt_runtime_helper_path,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| {
            format!("restricted CRT runtime startup helper failed before Rust probe: {e}")
        })?;
        println!(
            "[POC-CRT-RUNTIME-STARTUP] executable={} result=EXITED_ZERO exit_code={crt_runtime_helper_exit}",
            crt_runtime_helper_path.display()
        );

        let grouped_crt_helper_exit = launch_poc_native_startup_helper(
            restricted_token.0,
            &grouped_crt_helper_path,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &smoke_environment,
        )
        .map_err(|e| {
            format!("restricted grouped CRT startup helper failed before Rust probe: {e}")
        })?;
        println!(
            "[POC-GROUPED-CRT-STARTUP] executable={} result=EXITED_ZERO exit_code={grouped_crt_helper_exit}",
            grouped_crt_helper_path.display()
        );

        // 7. Launch restricted child and collect filesystem evidence.
        let child_evidence = launch_poc_restricted_child(
            restricted_token.0,
            &station_guard.as_ref().unwrap().station_name,
            &desktop_guard.name,
            &job,
            &fixture,
            controller_pid,
            cdb_path.as_deref(),
            &mut cdb_report,
        )
        .map_err(|e| format!("launch restricted child: {e}"))?;

        if !child_evidence.is_all_passed() {
            return Err(format!("child probe failed assertions: {child_evidence:?}"));
        }

        // 7. Unrestricted parent independently verifies canary integrity (content + metadata)
        fixture
            .verify_canaries_unmodified()
            .map_err(|e| format!("verify canaries: {e}"))?;

        Ok(())
    });

    let mut run_error = run_res.err();
    if let Some(guard) = station_guard.as_mut() {
        let restore_result = guard.restore();
        println!(
            "[POC-WINDOW-STATION] restore_attempted={} restore_succeeded={}",
            guard.restore_attempted,
            restore_result.is_ok()
        );
        if let Err(error) = restore_result {
            fixture.recovery_required = true;
            apply_cdb_acl_restore_result(&mut cdb_report, true);
            let station_restore_error = if cdb_path.is_some() {
                format!(
                    "restore Window Station DACL: {error}; CDB diagnostic classification={:?}",
                    CdbTraceClassification::AclRestoreFailed
                )
            } else {
                format!("restore Window Station DACL: {error}")
            };
            run_error = Some(match run_error {
                Some(previous) => format!("{previous}; {station_restore_error}"),
                None => station_restore_error,
            });
        }
    }

    if let Some(report) = cdb_report.as_ref() {
        println!("{}", report.render());
    }

    let outcome = fixture.handle_post_mutation_exit(run_error);

    if let Some(err) = outcome.error_message {
        panic!(
            "[FAIL] {err}; restore attempted: {}; restore succeeded: {}; fixture preserved: {}",
            outcome.restore_attempted, outcome.restore_succeeded, outcome.fixture_preserved
        );
    } else {
        println!("[POC-RESULT] PASS-FILESYSTEM-POC (Filesystem isolation PoC passed; NOT Stage 2 PASS; network isolation unresolved)");
    }
}
