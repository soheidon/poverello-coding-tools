$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$repoRoot = (Get-Item "$scriptDir\..\..").FullName

# Win32 helper for final filesystem path resolution and atomic directory creation
if (-not ([System.Management.Automation.PSTypeName]"PoverelloTestHarnessHelper").Type) {
    $helperDef = @"
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

public static class PoverelloTestHarnessHelper {
    private const uint GENERIC_READ = 0x80000000;
    private const uint FILE_SHARE_READ = 1;
    private const uint FILE_SHARE_WRITE = 2;
    private const uint FILE_SHARE_DELETE = 4;
    private const uint OPEN_EXISTING = 3;
    private const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
    private const uint FILE_NAME_NORMALIZED = 0x0;
    private const uint VOLUME_NAME_DOS = 0x0;

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    private static extern SafeFileHandle CreateFile(
        string lpFileName,
        uint dwDesiredAccess,
        uint dwShareMode,
        IntPtr lpSecurityAttributes,
        uint dwCreationDisposition,
        uint dwFlagsAndAttributes,
        IntPtr hTemplateFile
    );

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    private static extern uint GetFinalPathNameByHandle(
        SafeFileHandle hFile,
        StringBuilder lpszFilePath,
        uint cchFilePath,
        uint dwFlags
    );

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    public static extern bool CreateDirectory(string lpPathName, IntPtr lpSecurityAttributes);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    public struct STARTUPINFO {
        public uint cb;
        public string lpReserved;
        public string lpDesktop;
        public string lpTitle;
        public uint dwX;
        public uint dwY;
        public uint dwXSize;
        public uint dwYSize;
        public uint dwXCountChars;
        public uint dwYCountChars;
        public uint dwFillAttribute;
        public uint dwFlags;
        public ushort wShowWindow;
        public ushort cbReserved2;
        public IntPtr lpReserved2;
        public IntPtr hStdInput;
        public IntPtr hStdOutput;
        public IntPtr hStdError;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PROCESS_INFORMATION {
        public IntPtr hProcess;
        public IntPtr hThread;
        public uint dwProcessId;
        public uint dwThreadId;
    }

    private const uint STARTF_USESTDHANDLES = 0x00000100;
    private const uint STD_INPUT_HANDLE = unchecked((uint)-10);
    private const uint STD_OUTPUT_HANDLE = unchecked((uint)-11);
    private const uint STD_ERROR_HANDLE = unchecked((uint)-12);
    private const uint INFINITE = 0xFFFFFFFF;
    private const uint WAIT_OBJECT_0 = 0x00000000;
    private const uint WAIT_FAILED = 0xFFFFFFFF;

    public static bool InjectWaitFailure = false;
    public static bool InjectCreateProcessFailure = false;

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr GetStdHandle(uint nStdHandle);

    [DllImport("kernel32.dll", EntryPoint = "CreateProcessW", ExactSpelling = true, SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CreateProcessW(
        string lpApplicationName,
        StringBuilder lpCommandLine,
        IntPtr lpProcessAttributes,
        IntPtr lpThreadAttributes,
        bool bInheritHandles,
        uint dwCreationFlags,
        IntPtr lpEnvironment,
        string lpCurrentDirectory,
        ref STARTUPINFO lpStartupInfo,
        out PROCESS_INFORMATION lpProcessInformation
    );

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(IntPtr hHandle, uint dwMilliseconds);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetExitCodeProcess(IntPtr hProcess, out uint lpExitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr hObject);

    public static string EscapeArgument(string arg) {
        if (string.IsNullOrEmpty(arg)) return "\"\"";
        if (arg.IndexOfAny(new char[] { ' ', '\t', '\n', '\v', '\"' }) == -1) return arg;
        StringBuilder sb = new StringBuilder();
        sb.Append('\"');
        for (int i = 0; i < arg.Length; i++) {
            int backslashes = 0;
            while (i < arg.Length && arg[i] == '\\') {
                backslashes++;
                i++;
            }
            if (i == arg.Length) {
                sb.Append('\\', backslashes * 2);
                break;
            } else if (arg[i] == '\"') {
                sb.Append('\\', backslashes * 2 + 1);
                sb.Append('\"');
            } else {
                sb.Append('\\', backslashes);
                sb.Append(arg[i]);
            }
        }
        sb.Append('\"');
        return sb.ToString();
    }

    public static int RunProcessWithDesktop(string appPath, string[] args, string workDir, string desktop) {
        StringBuilder cmdLine = new StringBuilder();
        cmdLine.Append(EscapeArgument(appPath));
        if (args != null) {
            foreach (string arg in args) {
                cmdLine.Append(' ');
                cmdLine.Append(EscapeArgument(arg));
            }
        }

        STARTUPINFO si = new STARTUPINFO();
        si.cb = (uint)Marshal.SizeOf(typeof(STARTUPINFO));
        si.lpDesktop = desktop;
        si.dwFlags = STARTF_USESTDHANDLES;
        si.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
        si.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
        si.hStdError = GetStdHandle(STD_ERROR_HANDLE);

        if (InjectCreateProcessFailure) {
            throw new IOException("Injected CreateProcess failure (Win32 error 5: ERROR_ACCESS_DENIED)");
        }

        PROCESS_INFORMATION pi = new PROCESS_INFORMATION();
        bool success = CreateProcessW(
            null,
            cmdLine,
            IntPtr.Zero,
            IntPtr.Zero,
            true,
            0,
            IntPtr.Zero,
            workDir,
            ref si,
            out pi
        );
        if (!success) {
            int err = Marshal.GetLastWin32Error();
            throw new IOException("CreateProcess failed with Win32 error " + err + " for command line: " + cmdLine.ToString());
        }
        try {
            uint waitRes = WaitForSingleObject(pi.hProcess, INFINITE);
            if (InjectWaitFailure) {
                waitRes = WAIT_FAILED;
            }
            if (waitRes != WAIT_OBJECT_0) {
                int err = Marshal.GetLastWin32Error();
                throw new IOException("WaitForSingleObject failed with wait result 0x" + waitRes.ToString("X8") + ", Win32 error " + err);
            }
            uint exitCode;
            if (!GetExitCodeProcess(pi.hProcess, out exitCode)) {
                int err = Marshal.GetLastWin32Error();
                throw new IOException("GetExitCodeProcess failed with Win32 error " + err);
            }
            return (int)exitCode;
        } finally {
            if (pi.hProcess != IntPtr.Zero) CloseHandle(pi.hProcess);
            if (pi.hThread != IntPtr.Zero) CloseHandle(pi.hThread);
        }
    }

    public static string GetFinalPath(string path) {
        using (SafeFileHandle hFile = CreateFile(
            path,
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            IntPtr.Zero,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            IntPtr.Zero)) {
            if (hFile.IsInvalid) {
                int err = Marshal.GetLastWin32Error();
                throw new IOException("CreateFile failed for path \"" + path + "\" with Win32 error " + err);
            }
            StringBuilder sb = new StringBuilder(1024);
            uint res = GetFinalPathNameByHandle(hFile, sb, (uint)sb.Capacity, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
            if (res == 0) {
                int err = Marshal.GetLastWin32Error();
                throw new IOException("GetFinalPathNameByHandle failed for path \"" + path + "\" with Win32 error " + err);
            }
            string finalPath = sb.ToString();
            if (finalPath.StartsWith(@"\\?\UNC\")) {
                finalPath = @"\\" + finalPath.Substring(8);
            } else if (finalPath.StartsWith(@"\\?\")) {
                finalPath = finalPath.Substring(4);
            }
            return finalPath;
        }
    }
}
"@
    Add-Type -TypeDefinition $helperDef
}

function Assert-NoReparsePointChain([string]$path, [string]$label) {
    $curr = New-Object System.IO.DirectoryInfo($path)
    while ($curr -ne $null) {
        if (($curr.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$label or an ancestor is a reparse point: '$($curr.FullName)'"
        }
        $curr = $curr.Parent
    }
}

function Resolve-VerifiedDirectoryIdentity([string]$path, [string]$label) {
    if (-not [System.IO.Path]::IsPathRooted($path)) {
        throw "$label must be an absolute path: '$path'"
    }
    if (-not [System.IO.Directory]::Exists($path)) {
        throw "$label does not exist: '$path'"
    }
    # Check input path and all its ancestors for reparse points BEFORE resolving final path
    Assert-NoReparsePointChain $path $label

    $finalPath = [PoverelloTestHarnessHelper]::GetFinalPath($path).TrimEnd('\')
    if (-not [System.IO.Directory]::Exists($finalPath)) {
        throw "$label resolved to non-existent final path: '$finalPath'"
    }
    # Check resolved final path and all its ancestors as well
    Assert-NoReparsePointChain $finalPath "$label final target"
    return $finalPath
}

$suppliedRoot = $env:POVERELLO_TEST_TMP
$baseDir = $null

if ([string]::IsNullOrWhiteSpace($suppliedRoot)) {
    # Default behavior when POVERELLO_TEST_TMP is not supplied
    $baseDir = Join-Path $repoRoot "gui\src-tauri\target\test-tmp"
    if (-not (Test-Path $baseDir)) {
        [System.IO.Directory]::CreateDirectory($baseDir) | Out-Null
    }
} else {
    # Strict validation of explicit POVERELLO_TEST_TMP root
    $canonicalSupplied = Resolve-VerifiedDirectoryIdentity $suppliedRoot "Supplied POVERELLO_TEST_TMP"
    $canonicalRepo = Resolve-VerifiedDirectoryIdentity $repoRoot "Repository root"

    # Enforce bidirectional disjointness between test root and repository root
    if ($canonicalSupplied.Equals($canonicalRepo, [System.StringComparison]::OrdinalIgnoreCase) -or
        $canonicalSupplied.StartsWith($canonicalRepo + "\", [System.StringComparison]::OrdinalIgnoreCase) -or
        $canonicalRepo.StartsWith($canonicalSupplied + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Supplied POVERELLO_TEST_TMP ('$canonicalSupplied') overlaps with repository root ('$canonicalRepo')"
    }

    # Verify writability via exclusive file creation, cleaning up in finally
    $probeGuid = [System.Guid]::NewGuid().ToString("N").Substring(0, 8)
    $probeFile = Join-Path $canonicalSupplied ".write-probe-$probeGuid.tmp"
    $probeCreated = $false
    try {
        $fs = [System.IO.File]::Open($probeFile, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
        $fs.Close()
        $fs.Dispose()
        $probeCreated = $true
    } catch {
        throw "Supplied POVERELLO_TEST_TMP is not writable (failed exclusive create on probe file '$probeFile'): $_"
    } finally {
        if ($probeCreated) {
            try {
                [System.IO.File]::Delete($probeFile)
            } catch {
                throw "Failed to clean up writability probe file at '$probeFile': $_"
            }
        }
    }

    $baseDir = $canonicalSupplied
}

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$shortGuid = [System.Guid]::NewGuid().ToString("N").Substring(0, 8)
$injectedName = $env:POVERELLO_TEST_INJECT_RUN_DIR_NAME
$runDirName = $null
if ([string]::IsNullOrWhiteSpace($injectedName)) {
    $runDirName = "r-$shortGuid"
} else {
    # Reject separators, path traversal, drive prefixes, rooted paths, and non-alphanumeric characters
    if ($injectedName -match '[\\/:]' -or
        $injectedName -like "*..*" -or
        [System.IO.Path]::IsPathRooted($injectedName) -or
        $injectedName -notmatch '^[a-zA-Z0-9_\-]+$') {
        throw "Invalid POVERELLO_TEST_INJECT_RUN_DIR_NAME '$injectedName': must be a single non-empty basename containing only alphanumeric characters, underscores, or dashes with no path separators or traversal"
    }
    $runDirName = $injectedName
}

# Verify prospective child path is strictly a direct child of $baseDir
$runRoot = [System.IO.Path]::Combine($baseDir, $runDirName)
$parentDir = [System.IO.Path]::GetDirectoryName($runRoot)
if (-not $parentDir.Equals($baseDir, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Run root directory '$runRoot' is not a direct child of base directory '$baseDir'"
}

# Atomic create-new semantics: fail closed if directory or file already exists
$created = [PoverelloTestHarnessHelper]::CreateDirectory($runRoot, [IntPtr]::Zero)
if (-not $created) {
    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
    throw "Run directory creation failed with error $err (collision on '$runRoot')"
}

# Verify the created child is an ordinary non-reparse directory
$runDirInfo = New-Object System.IO.DirectoryInfo($runRoot)
if (-not $runDirInfo.Exists) {
    throw "Run directory '$runRoot' does not exist after creation"
}
if (($runDirInfo.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "Created run directory '$runRoot' is a reparse point"
}

# Also verify the canonical physical final path of the created directory has canonical $baseDir as parent
$finalRunRoot = [PoverelloTestHarnessHelper]::GetFinalPath($runRoot).TrimEnd('\')
$finalParent = [System.IO.Path]::GetDirectoryName($finalRunRoot)
$canonicalBase = [PoverelloTestHarnessHelper]::GetFinalPath($baseDir).TrimEnd('\')
if (-not $finalParent.Equals($canonicalBase, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Resolved final run directory '$finalRunRoot' parent does not match canonical base directory '$canonicalBase'"
}

$validateOnly = $args -contains "-ValidateOnly"
$validateUnicodeLaunch = $args -contains "-ValidateUnicodeLaunch"
$cargoArgs = @($args | Where-Object { $_ -ne "-ValidateOnly" -and $_ -ne "-ValidateUnicodeLaunch" })

if ($validateUnicodeLaunch) {
    $unicodeMarker = [char]::ConvertFromUtf32(0x1F9EA)
    $unicodeDirectoryName = "unicode-$unicodeMarker-$shortGuid"
    $unicodeDirectory = Join-Path $runRoot $unicodeDirectoryName
    $unicodeScript = Join-Path $unicodeDirectory "verify-unicode-launch.ps1"
    $unicodeArgument = "probe-$unicodeMarker argument"
    $expectedCwdBase64 = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($unicodeDirectory))
    $expectedArgumentBase64 = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($unicodeArgument))
    $childScriptText = @'
param([string]$ProbeArgument)
$expectedCwd = [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('__EXPECTED_CWD__'))
$expectedArgument = [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('__EXPECTED_ARGUMENT__'))
$actualCwd = [IO.Path]::GetFullPath((Get-Location).ProviderPath)
if (-not [string]::Equals($actualCwd, $expectedCwd, [StringComparison]::OrdinalIgnoreCase)) {
    [Console]::Error.WriteLine("Working directory mismatch: '$actualCwd'")
    exit 41
}
if (-not [string]::Equals($ProbeArgument, $expectedArgument, [StringComparison]::Ordinal)) {
    [Console]::Error.WriteLine('Unicode argument mismatch')
    exit 42
}
[Console]::WriteLine('POVERELLO_UNICODE_LAUNCH_OK')
exit 0
'@
    $childScriptText = $childScriptText.Replace('__EXPECTED_CWD__', $expectedCwdBase64).Replace('__EXPECTED_ARGUMENT__', $expectedArgumentBase64)
    $unicodeFixtureCreated = $false
    $unicodeLaunchSucceeded = $false
    try {
        [System.IO.Directory]::CreateDirectory($unicodeDirectory) | Out-Null
        $unicodeFixtureCreated = $true
        [System.IO.File]::WriteAllText($unicodeScript, $childScriptText, [Text.Encoding]::ASCII)

        $powershellExe = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        if (-not (Test-Path -LiteralPath $powershellExe -PathType Leaf)) {
            throw "Windows PowerShell executable not found: '$powershellExe'"
        }

        Write-Output '[TEST_RUNNER_LAUNCH_MODE: WinSta0\Default UnicodeValidation]'
        $unicodeExitCode = [PoverelloTestHarnessHelper]::RunProcessWithDesktop(
            $powershellExe,
            @('-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $unicodeScript, $unicodeArgument),
            $unicodeDirectory,
            'WinSta0\Default'
        )
        if ($unicodeExitCode -ne 0) {
            throw "Unicode CreateProcessW smoke exited with code $unicodeExitCode; fixture preserved at '$unicodeDirectory'"
        }
        $unicodeLaunchSucceeded = $true
        Write-Output 'Unicode CreateProcessW working-directory and argument validation: PASS'
    }
    finally {
        if ($unicodeLaunchSucceeded -and $unicodeFixtureCreated) {
            try {
                [System.IO.File]::Delete($unicodeScript)
                $remainingEntries = @(Get-ChildItem -LiteralPath $unicodeDirectory -Force)
                if ($remainingEntries.Count -ne 0) {
                    throw "Unexpected files remain in owned Unicode fixture '$unicodeDirectory'"
                }
                [System.IO.Directory]::Delete($unicodeDirectory, $false)
                $remainingRunEntries = @(Get-ChildItem -LiteralPath $runRoot -Force)
                if ($remainingRunEntries.Count -eq 0) {
                    [System.IO.Directory]::Delete($runRoot, $false)
                }
            } catch {
                throw "Unicode launch succeeded, but owned fixture cleanup failed; inspect '$unicodeDirectory': $_"
            }
        } elseif ($unicodeFixtureCreated) {
            Write-Warning "Preserving Unicode launch diagnostic fixture at '$unicodeDirectory'"
        }
    }
    exit 0
}

if ($validateOnly) {
    Write-Output "POVERELLO_TEST_TMP_VALIDATED:$runRoot"
    exit 0
}

$origTemp = $env:TEMP
$origTmp = $env:TMP
$origPoverelloTestTmp = $env:POVERELLO_TEST_TMP
$origGitCount = $env:GIT_CONFIG_COUNT
$origGitKey0 = $env:GIT_CONFIG_KEY_0
$origGitVal0 = $env:GIT_CONFIG_VALUE_0
$origPath = $env:PATH

$exitCode = 0
try {
    $env:TEMP = $runRoot
    $env:TMP = $runRoot
    $env:POVERELLO_TEST_TMP = $runRoot

    # Enable Git long paths support for deep nested submodule paths on Windows
    $env:GIT_CONFIG_COUNT = "1"
    $env:GIT_CONFIG_KEY_0 = "core.longpaths"
    $env:GIT_CONFIG_VALUE_0 = "true"
    # Ensure official Git for Windows precedes any MSYS git to avoid MSYS 0xC0000022 sandbox restrictions
    $cleanPath = ($env:PATH -split ';' | Where-Object { $_ -notlike '*msys64*' -and $_ -ne 'C:\Program Files\Git\cmd' }) -join ';'
    $env:PATH = "C:\Program Files\Git\cmd;$cleanPath"

    Push-Location $repoRoot
    try {
        $manifestPath = "gui/src-tauri/Cargo.toml"
        $cmdArgs = @("test", "--offline", "--manifest-path", $manifestPath)
        if ($cargoArgs) {
            $cmdArgs += $cargoArgs
        }

        $cargoCmd = (Get-Command cargo.exe -ErrorAction SilentlyContinue)
        $cargoExe = if ($cargoCmd) {
            $cargoCmd.Source
        } elseif (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe") {
            "$env:USERPROFILE\.cargo\bin\cargo.exe"
        } else {
            "cargo"
        }

        $runningOnWindows = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
        if ($runningOnWindows) {
            Write-Output "[TEST_RUNNER_LAUNCH_MODE: WinSta0\Default]"
            $exitCode = [PoverelloTestHarnessHelper]::RunProcessWithDesktop($cargoExe, $cmdArgs, $repoRoot, "WinSta0\Default")
        } else {
            Write-Output "[TEST_RUNNER_LAUNCH_MODE: DirectCargo]"
            & cargo @cmdArgs
            $exitCode = $LASTEXITCODE
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    $env:TEMP = $origTemp
    $env:TMP = $origTmp
    $env:POVERELLO_TEST_TMP = $origPoverelloTestTmp
    $env:GIT_CONFIG_COUNT = $origGitCount
    $env:GIT_CONFIG_KEY_0 = $origGitKey0
    $env:GIT_CONFIG_VALUE_0 = $origGitVal0
    $env:PATH = $origPath
}

exit $exitCode
