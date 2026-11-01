<#
.SYNOPSIS
    Poverello Codex Workflow Skills - Multi-Reviewer Dispatcher Helper.
.DESCRIPTION
    Dispatches fixed read-only reviewers (codex, gemini, deepseek) against a synthesized
    snapshot directory in parallel. Enforces Windows Job Object confinement (CREATE_SUSPENDED
    -> AssignProcessToJobObject -> ResumeThread), strict pipe handle inheritance controls,
    refusal of unverified Gemini sandbox environments (UNAVAILABLE), positional task dispatch
    for DeepSeek Harness, process-tree termination on timeout/cancel, stdout capping (1 MiB),
    and strictly discards stderr reasoning text.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SnapshotDir,

    [Parameter(Mandatory = $false)]
    [string]$TargetDigest = "",

    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [string[]]$Reviewers,

    [Parameter(Mandatory = $false)]
    [int]$TimeoutSeconds = 300,

    [Parameter(Mandatory = $false)]
    [string]$ReviewPrompt = "Review the plan and implementation in this workspace against approved specifications. Do not modify files or execute stateful commands."
)

$ErrorActionPreference = "Stop"

# 1. Parameter & Path Validations
if (-not (Test-Path -LiteralPath $SnapshotDir -PathType Container)) {
    $errObj = @{
        error = "Snapshot directory does not exist or is not a directory: $SnapshotDir"
        results = @()
    }
    $errObj | ConvertTo-Json -Compress
    exit 1
}

# Verify snapshot path and any contained item is not a symlink / reparse point
$dirItem = Get-Item -LiteralPath $SnapshotDir -Force
if (($dirItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
    $errObj = @{
        error = "Snapshot directory is a reparse point or symbolic link: $SnapshotDir"
        results = @()
    }
    $errObj | ConvertTo-Json -Compress
    exit 1
}

$reparseChildren = Get-ChildItem -LiteralPath $SnapshotDir -Recurse -Force -ErrorAction SilentlyContinue | Where-Object {
    ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
}
if ($reparseChildren -and $reparseChildren.Count -gt 0) {
    $firstOffender = $reparseChildren[0].FullName
    $errObj = @{
        error = "Snapshot directory contains reparse point or symbolic link: $firstOffender"
        results = @()
    }
    $errObj | ConvertTo-Json -Compress
    exit 1
}

$validReviewerIds = @("codex", "gemini", "deepseek")
$targetReviewers = @()
foreach ($r in $Reviewers) {
    if ([string]::IsNullOrWhiteSpace($r) -or $r.Trim() -eq "@()") {
        continue
    }
    $rClean = $r.Trim().ToLowerInvariant()
    if ($validReviewerIds -notcontains $rClean) {
        $errObj = @{
            error = "Invalid reviewer ID: '$r'. Allowed: $($validReviewerIds -join ', ')"
            results = @()
        }
        $errObj | ConvertTo-Json -Compress
        exit 1
    }
    if ($targetReviewers -notcontains $rClean) {
        $targetReviewers += $rClean
    }
}

if ($targetReviewers.Count -eq 0) {
    $outObj = @{
        status = "REVIEW_SKIPPED"
        results = @()
    }
    $outObj | ConvertTo-Json -Compress
    exit 0
}

# 2. Define Windows Job Object Process Runner in C#
$csharpRunnerSource = @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;

public class PoverelloJobExecutionResult {
    public int ExitCode;
    public string Stdout;
    public bool HasStderr;
    public bool TimedOut;
    public bool IsTruncated;
    public string Error;
}

public static class PoverelloProcessJobRunner {
    [StructLayout(LayoutKind.Sequential)]
    struct PROCESS_INFORMATION {
        public IntPtr hProcess;
        public IntPtr hThread;
        public int dwProcessId;
        public int dwThreadId;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct STARTUPINFO {
        public int cb;
        public string lpReserved;
        public string lpDesktop;
        public string lpTitle;
        public int dwX;
        public int dwY;
        public int dwXSize;
        public int dwYSize;
        public int dwXCountChars;
        public int dwYCountChars;
        public int dwFillAttribute;
        public int dwFlags;
        public short wShowWindow;
        public short cbReserved2;
        public IntPtr lpReserved2;
        public IntPtr hStdInput;
        public IntPtr hStdOutput;
        public IntPtr hStdError;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct SECURITY_ATTRIBUTES {
        public int nLength;
        public IntPtr lpSecurityDescriptor;
        public bool bInheritHandle;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct IO_COUNTERS {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryLimit;
        public UIntPtr PeakJobMemoryLimit;
    }

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr CreateJobObject(IntPtr lpJobAttributes, string lpName);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool SetInformationJobObject(IntPtr hJob, int JobObjectInfoClass, IntPtr lpJobObjectInfo, uint cbJobObjectInfoLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool AssignProcessToJobObject(IntPtr hJob, IntPtr hProcess);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool TerminateJobObject(IntPtr hJob, uint uExitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool CreatePipe(out IntPtr hReadPipe, out IntPtr hWritePipe, ref SECURITY_ATTRIBUTES lpPipeAttributes, int nSize);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool SetHandleInformation(IntPtr hObject, uint dwMask, uint dwFlags);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool CreateProcess(
        string lpApplicationName,
        string lpCommandLine,
        ref SECURITY_ATTRIBUTES lpProcessAttributes,
        ref SECURITY_ATTRIBUTES lpThreadAttributes,
        bool bInheritHandles,
        uint dwCreationFlags,
        IntPtr lpEnvironment,
        string lpCurrentDirectory,
        ref STARTUPINFO lpStartupInfo,
        out PROCESS_INFORMATION lpProcessInformation
    );

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern uint ResumeThread(IntPtr hThread);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern uint WaitForSingleObject(IntPtr hHandle, uint dwMilliseconds);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool GetExitCodeProcess(IntPtr hProcess, out int lpExitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool TerminateProcess(IntPtr hProcess, uint uExitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool CloseHandle(IntPtr hObject);

    const int JobObjectExtendedLimitInformation = 9;
    const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
    const uint CREATE_SUSPENDED = 0x00000004;
    const uint CREATE_NO_WINDOW = 0x08000000;
    const int STARTF_USESTDHANDLES = 0x00000100;
    const uint HANDLE_FLAG_INHERIT = 0x00000001;
    const uint WAIT_OBJECT_0 = 0x00000000;
    const uint WAIT_TIMEOUT = 0x00000102;

    private static string EscapeArg(string arg) {
        if (string.IsNullOrEmpty(arg)) return "\"\"";
        if (!arg.Contains(" ") && !arg.Contains("\t") && !arg.Contains("\"")) return arg;
        return "\"" + arg.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";
    }

    public static PoverelloJobExecutionResult RunProcessInJob(string appPath, string[] args, string workingDirectory, int timeoutSeconds, int maxStdoutBytes) {
        var res = new PoverelloJobExecutionResult {
            ExitCode = -1,
            Stdout = "",
            HasStderr = false,
            TimedOut = false,
            IsTruncated = false,
            Error = null
        };

        IntPtr hJob = CreateJobObject(IntPtr.Zero, null);
        if (hJob == IntPtr.Zero) {
            res.Error = "CreateJobObject failed: " + Marshal.GetLastWin32Error();
            return res;
        }

        try {
            var extInfo = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
            extInfo.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            int length = Marshal.SizeOf(typeof(JOBOBJECT_EXTENDED_LIMIT_INFORMATION));
            IntPtr pInfo = Marshal.AllocHGlobal(length);
            try {
                Marshal.StructureToPtr(extInfo, pInfo, false);
                if (!SetInformationJobObject(hJob, JobObjectExtendedLimitInformation, pInfo, (uint)length)) {
                    res.Error = "SetInformationJobObject failed: " + Marshal.GetLastWin32Error();
                    return res;
                }
            } finally {
                Marshal.FreeHGlobal(pInfo);
            }

            var sa = new SECURITY_ATTRIBUTES();
            sa.nLength = Marshal.SizeOf(typeof(SECURITY_ATTRIBUTES));
            sa.bInheritHandle = true;

            IntPtr hOutRead, hOutWrite;
            if (!CreatePipe(out hOutRead, out hOutWrite, ref sa, 0)) {
                res.Error = "CreatePipe (stdout) failed: " + Marshal.GetLastWin32Error();
                return res;
            }
            // Ensure parent read pipe handle is NOT inherited by child
            SetHandleInformation(hOutRead, HANDLE_FLAG_INHERIT, 0);

            IntPtr hErrRead, hErrWrite;
            if (!CreatePipe(out hErrRead, out hErrWrite, ref sa, 0)) {
                CloseHandle(hOutRead);
                CloseHandle(hOutWrite);
                res.Error = "CreatePipe (stderr) failed: " + Marshal.GetLastWin32Error();
                return res;
            }
            // Ensure parent read pipe handle is NOT inherited by child
            SetHandleInformation(hErrRead, HANDLE_FLAG_INHERIT, 0);

            var si = new STARTUPINFO();
            si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
            si.dwFlags = STARTF_USESTDHANDLES;
            si.hStdOutput = hOutWrite;
            si.hStdError = hErrWrite;
            si.hStdInput = IntPtr.Zero;

            var pi = new PROCESS_INFORMATION();
            var cmdBuilder = new StringBuilder();
            cmdBuilder.Append(EscapeArg(appPath));
            if (args != null) {
                foreach (var a in args) {
                    cmdBuilder.Append(" ");
                    cmdBuilder.Append(EscapeArg(a));
                }
            }
            string cmdLine = cmdBuilder.ToString();

            var procSec = new SECURITY_ATTRIBUTES();
            procSec.nLength = Marshal.SizeOf(typeof(SECURITY_ATTRIBUTES));
            var threadSec = new SECURITY_ATTRIBUTES();
            threadSec.nLength = Marshal.SizeOf(typeof(SECURITY_ATTRIBUTES));

            bool created = CreateProcess(
                null,
                cmdLine,
                ref procSec,
                ref threadSec,
                true,
                CREATE_SUSPENDED | CREATE_NO_WINDOW,
                IntPtr.Zero,
                string.IsNullOrEmpty(workingDirectory) ? null : workingDirectory,
                ref si,
                out pi
            );
            int createProcessErr = created ? 0 : Marshal.GetLastWin32Error();

            CloseHandle(hOutWrite);
            CloseHandle(hErrWrite);

            if (!created) {
                CloseHandle(hOutRead);
                CloseHandle(hErrRead);
                res.Error = "CreateProcess failed (" + createProcessErr + "): " + appPath;
                return res;
            }

            try {
                if (!AssignProcessToJobObject(hJob, pi.hProcess)) {
                    int err = Marshal.GetLastWin32Error();
                    TerminateProcess(pi.hProcess, 1);
                    res.Error = "AssignProcessToJobObject failed (" + err + ")";
                    return res;
                }

                if (ResumeThread(pi.hThread) == 0xFFFFFFFF) {
                    int err = Marshal.GetLastWin32Error();
                    TerminateProcess(pi.hProcess, 1);
                    res.Error = "ResumeThread failed (" + err + ")";
                    return res;
                }

                var stdoutStream = new FileStream(new Microsoft.Win32.SafeHandles.SafeFileHandle(hOutRead, true), FileAccess.Read);
                var stderrStream = new FileStream(new Microsoft.Win32.SafeHandles.SafeFileHandle(hErrRead, true), FileAccess.Read);

                var memOut = new MemoryStream();
                bool isTruncated = false;
                var outTask = Task.Run(() => {
                    byte[] buf = new byte[4096];
                    int r;
                    int total = 0;
                    while ((r = stdoutStream.Read(buf, 0, buf.Length)) > 0) {
                        if (total + r <= maxStdoutBytes) {
                            memOut.Write(buf, 0, r);
                            total += r;
                        } else {
                            int avail = maxStdoutBytes - total;
                            if (avail > 0) {
                                memOut.Write(buf, 0, avail);
                                total += avail;
                            }
                            isTruncated = true;
                        }
                    }
                });

                bool hasStderr = false;
                var errTask = Task.Run(() => {
                    byte[] buf = new byte[4096];
                    int r;
                    while ((r = stderrStream.Read(buf, 0, buf.Length)) > 0) {
                        hasStderr = true;
                    }
                });

                uint waitRes = WaitForSingleObject(pi.hProcess, (uint)(timeoutSeconds * 1000));
                if (waitRes == WAIT_TIMEOUT) {
                    res.TimedOut = true;
                    TerminateJobObject(hJob, 1);
                    uint termWait = WaitForSingleObject(pi.hProcess, 5000);
                    if (termWait != WAIT_OBJECT_0) {
                        res.Error = "Process termination could not be verified after timeout";
                    }
                } else if (waitRes == WAIT_OBJECT_0) {
                    int ec;
                    if (GetExitCodeProcess(pi.hProcess, out ec)) {
                        res.ExitCode = ec;
                    }
                } else {
                    res.Error = "WaitForSingleObject failed: " + Marshal.GetLastWin32Error();
                }

                Task.WaitAll(new Task[] { outTask, errTask }, 2000);
                res.Stdout = Encoding.UTF8.GetString(memOut.ToArray());
                res.IsTruncated = isTruncated;
                res.HasStderr = hasStderr;

                return res;
            } finally {
                CloseHandle(pi.hThread);
                CloseHandle(pi.hProcess);
            }
        } finally {
            CloseHandle(hJob);
        }
    }
}
'@

if (-not ([System.Management.Automation.PSTypeName]'PoverelloProcessJobRunner').Type) {
    Add-Type -TypeDefinition $csharpRunnerSource
}

# 3. Reviewer Command Line Builders
function Get-ReviewerCommand {
    param([string]$ReviewerId, [string]$Prompt, [string]$WorkDir)
    switch ($ReviewerId) {
        "codex" {
            return @{
                Command = "codex"
                Args = @("exec", "--ephemeral", "--sandbox", "read-only", $Prompt)
                UnavailableReason = $null
            }
        }
        "gemini" {
            # Gemini CLI must be strictly isolated.
            # In the absence of an officially verified and enforced read-only sandbox on Windows,
            # fail-closed and report UNAVAILABLE to prevent write operations and YOLO transitions.
            $sandboxEnforced = $false
            if ($env:GEMINI_CLI_SANDBOX_ENFORCED -eq "1" -or (Test-Path -LiteralPath (Join-Path $WorkDir ".gemini-sandbox-verified"))) {
                $sandboxEnforced = $true
            }

            if (-not $sandboxEnforced) {
                return @{
                    Command = $null
                    Args = @()
                    UnavailableReason = "Gemini CLI read-only sandbox environment cannot be verified on this platform; reviewer is UNAVAILABLE"
                }
            }

            return @{
                Command = "gemini"
                Args = @("-p", $Prompt, "--approval-mode=plan", "--sandbox")
                UnavailableReason = $null
            }
        }
        "deepseek" {
            # Official headless dsh syntax takes positional task argument without -p
            return @{
                Command = "dsh"
                Args = @("--profile", "headless", "--permission-mode", "read-only", "--approval-policy", "never", $Prompt)
                UnavailableReason = $null
            }
        }
        default {
            throw "Unknown reviewer ID $ReviewerId"
        }
    }
}

# 4. Launch Reviewers in Parallel via PowerShell Background Jobs
$jobs = @()
$immediateResults = @()
$maxStdoutBytes = 1048576 # 1 MiB cap

foreach ($revId in $targetReviewers) {
    $cmdInfo = Get-ReviewerCommand -ReviewerId $revId -Prompt $ReviewPrompt -WorkDir $SnapshotDir

    if ($cmdInfo.Command -eq $null) {
        # Reviewer cannot be safely dispatched (e.g. Gemini without verified sandbox) -> fail-closed UNAVAILABLE
        $immediateResults += @{
            reviewer = $revId
            exit_code = -1
            stdout = ""
            has_stderr = $false
            timed_out = $false
            is_truncated = $false
            error = $cmdInfo.UnavailableReason
        }
        continue
    }

    $scriptBlock = {
        param($cmd, $argsList, $workDir, $timeoutSec, $maxBytes, $typeDef)
        if (-not ([System.Management.Automation.PSTypeName]'PoverelloProcessJobRunner').Type) {
            Add-Type -TypeDefinition $typeDef
        }
        $res = [PoverelloProcessJobRunner]::RunProcessInJob($cmd, [string[]]$argsList, $workDir, $timeoutSec, $maxBytes)
        return @{
            exit_code = $res.ExitCode
            stdout = $res.Stdout
            has_stderr = [bool]$res.HasStderr
            timed_out = [bool]$res.TimedOut
            is_truncated = [bool]$res.IsTruncated
            error = $res.Error
        }
    }

    $psJob = Start-Job -ScriptBlock $scriptBlock -ArgumentList @($cmdInfo.Command, $cmdInfo.Args, $SnapshotDir, $TimeoutSeconds, $maxStdoutBytes, $csharpRunnerSource)
    $jobs += @{
        ReviewerId = $revId
        Job = $psJob
    }
}

# 5. Await All Jobs with Overall Timeout
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$results = @()
$results += $immediateResults

foreach ($item in $jobs) {
    $revId = $item.ReviewerId
    $psJob = $item.Job

    $remainingMs = [Math]::Max(0, ($TimeoutSeconds * 1000) - $sw.ElapsedMilliseconds)
    $completed = Wait-Job -Job $psJob -Timeout ([int]($remainingMs / 1000))

    if ($completed) {
        $jobResult = Receive-Job -Job $psJob
        Remove-Job -Job $psJob -Force
        $results += @{
            reviewer = $revId
            exit_code = $jobResult.exit_code
            stdout = $jobResult.stdout
            has_stderr = [bool]$jobResult.has_stderr
            timed_out = [bool]$jobResult.timed_out
            is_truncated = [bool]$jobResult.is_truncated
            error = $jobResult.error
        }
    } else {
        # Outer PowerShell Job timed out - kill job and force cleanup
        Stop-Job -Job $psJob -ErrorAction SilentlyContinue
        Remove-Job -Job $psJob -Force
        $results += @{
            reviewer = $revId
            exit_code = -1
            stdout = ""
            has_stderr = $false
            timed_out = $true
            is_truncated = $false
            error = "Execution timed out after $TimeoutSeconds seconds"
        }
    }
}

$output = @{
    snapshot_dir = $SnapshotDir
    target_digest = $TargetDigest
    results = $results
}

$output | ConvertTo-Json -Depth 5 -Compress
