param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("validate-profile", "verify-verdict", "check-permission", "extract-allowlist", "status")]
    [string]$Subcommand,

    [Parameter(Mandatory = $false)]
    [string]$StateFile = "",

    [Parameter(Mandatory = $false)]
    [string]$PayloadJson = "",

    [Parameter(Mandatory = $false)]
    [string]$PayloadFile = "",

    [Parameter(Mandatory = $false)]
    [string]$ActionBit = "",

    [Parameter(Mandatory = $false)]
    [string]$BinaryPath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-AnthroBridgeBinary {
    param([string]$ExplicitPath)
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        if (Test-Path -LiteralPath $ExplicitPath) {
            return (Resolve-Path -LiteralPath $ExplicitPath).Path
        }
        throw "Specified BinaryPath does not exist: $ExplicitPath"
    }

    $repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")
    $candidates = @(
        (Join-Path $repoRoot "gui\src-tauri\target\debug\anthro-bridge.exe"),
        (Join-Path $repoRoot "gui\src-tauri\target\release\anthro-bridge.exe"),
        (Join-Path $repoRoot "gui\src-tauri\target\debug\anthro-bridge"),
        (Join-Path $repoRoot "gui\src-tauri\target\release\anthro-bridge")
    )

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    $cmd = Get-Command "anthro-bridge.exe" -ErrorAction SilentlyContinue
    if ($null -ne $cmd) {
        return $cmd.Source
    }

    throw "Could not locate anthro-bridge executable for auto-converge-guard."
}

try {
    $exe = Resolve-AnthroBridgeBinary -ExplicitPath $BinaryPath
    $cliArgs = @("auto-converge-guard", $Subcommand)

    if (-not [string]::IsNullOrWhiteSpace($StateFile)) {
        $cliArgs += @("--state-file", $StateFile)
    }
    if (-not [string]::IsNullOrWhiteSpace($PayloadFile)) {
        $cliArgs += @("--payload-file", $PayloadFile)
    }
    if (-not [string]::IsNullOrWhiteSpace($PayloadJson)) {
        $cliArgs += @("--payload-json", $PayloadJson)
    }
    if (-not [string]::IsNullOrWhiteSpace($ActionBit)) {
        $cliArgs += @("--action", $ActionBit)
    }

    $output = & $exe @cliArgs 2>&1
    $exitCode = $LASTEXITCODE
    if ($null -ne $output) {
        Write-Output ($output -join "`n")
    }
    exit $exitCode
}
catch {
    Write-Error $_.Exception.Message
    exit 1
}
