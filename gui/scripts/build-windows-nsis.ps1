param(
  [switch]$ReuseGeneratedBundle,
  [switch]$RequireUpdaterSigning,
  [switch]$PreflightOnly,
  [ValidateSet('Stable', 'Staging')][string]$BuildMode = 'Stable',
  [string]$TauriConfigOverlay,
  [string]$BuildOutputRoot
)

$ErrorActionPreference = 'Stop'
$script:buildLogPath = $null
$script:transcriptStarted = $false
trap {
  $originalError = $_
  $nativeExitCodeAtFailure = $LASTEXITCODE
  if ($script:transcriptStarted) {
    try {
      Write-Output 'BUILD FAILURE: terminating PowerShell error captured by build-helper trap.'
      Write-Output "Exception: $($originalError.Exception.Message)"
      Write-Output "FullyQualifiedErrorId: $($originalError.FullyQualifiedErrorId)"
      Write-Output "Script: $($originalError.InvocationInfo.ScriptName)"
      Write-Output "Line: $($originalError.InvocationInfo.ScriptLineNumber)"
      Write-Output "Command: $($originalError.InvocationInfo.Line.Trim())"
      Write-Output "Position: $($originalError.InvocationInfo.PositionMessage)"
      Write-Output "LASTEXITCODE at trap entry (may be stale; native-command association is unknown): $nativeExitCodeAtFailure"
      Stop-Transcript | Out-Null
    } catch { }
    $script:transcriptStarted = $false
  }
  throw $originalError
}

function Get-FileSha256Hex {
  param([Parameter(Mandatory = $true)][string]$Path)

  $stream = $null
  $sha256 = $null
  try {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    $digestBytes = $sha256.ComputeHash($stream)
    return [System.BitConverter]::ToString($digestBytes).Replace('-', '').ToUpperInvariant()
  }
  finally {
    if ($null -ne $stream) { $stream.Dispose() }
    if ($null -ne $sha256) { $sha256.Dispose() }
  }
}

$guiRoot = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $guiRoot 'src-tauri'
$canonicalDefaultTarget = [System.IO.Path]::GetFullPath((Join-Path $tauriRoot 'target'))
$appConfig = Get-Content -LiteralPath (Join-Path $tauriRoot 'tauri.conf.json') -Raw | ConvertFrom-Json
$productName = [string]$appConfig.productName
$appVersion = [string]$appConfig.version
if ($productName -ne 'Poverello Coding Tools' -or [string]::IsNullOrWhiteSpace($appVersion)) {
  throw "Unexpected stable product metadata: product='$productName', version='$appVersion'"
}

$RequireUpdaterSigning = $true
$expectedChannel = if ($BuildMode -eq 'Stable') { 'stable' } else { 'dev' }
$expectedMode = $BuildMode.ToLowerInvariant()
$channelAtStart = [Environment]::GetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'Process')
$modeAtStart = [Environment]::GetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', 'Process')
if ($channelAtStart -ne $expectedChannel) {
  throw "[$BuildMode] requires ANTHRO_BRIDGE_CHANNEL='$expectedChannel', but got '$channelAtStart'."
}
if (-not [string]::IsNullOrWhiteSpace($modeAtStart) -and $modeAtStart -ne $expectedMode) {
  throw "[$BuildMode] requires PCT_UPDATER_BUILD_MODE='$expectedMode', but got '$modeAtStart'."
}
if ([string]::IsNullOrWhiteSpace($modeAtStart)) {
  [Environment]::SetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', $expectedMode, 'Process')
}
if ($PreflightOnly -and -not $RequireUpdaterSigning) {
  throw '[-PreflightOnly] Preflight-only mode requires updater signing.'
}

# --- INS-005 Section C: Staging output root & config overlay validation ---
$targetRoot = $null
$isStagingBuild = $false

$hasBuildOutputRoot = -not [string]::IsNullOrWhiteSpace($BuildOutputRoot)
$hasTauriConfigOverlay = -not [string]::IsNullOrWhiteSpace($TauriConfigOverlay)

if ($BuildMode -eq 'Staging' -and -not $hasBuildOutputRoot) {
  throw 'Staging mode requires -BuildOutputRoot.'
}
if ($BuildMode -eq 'Staging' -and -not $hasTauriConfigOverlay) {
  throw 'Staging mode requires -TauriConfigOverlay.'
}
if ($BuildMode -eq 'Stable' -and ($hasBuildOutputRoot -or $hasTauriConfigOverlay)) {
  throw 'Stable mode does not accept staging overlay/output parameters.'
}
if ($hasBuildOutputRoot -and -not $hasTauriConfigOverlay) {
  throw 'Staging mode requires -TauriConfigOverlay when -BuildOutputRoot is specified.'
}
if ($hasTauriConfigOverlay -and -not $hasBuildOutputRoot) {
  throw 'Staging mode requires -BuildOutputRoot when -TauriConfigOverlay is specified.'
}

if ($hasBuildOutputRoot -and $hasTauriConfigOverlay) {
  if (-not [System.IO.Path]::IsPathRooted($BuildOutputRoot)) {
    throw "BuildOutputRoot must be an absolute path: $BuildOutputRoot"
  }
  $canonicalOutputRoot = [System.IO.Path]::GetFullPath($BuildOutputRoot)
  $canonicalRepoRoot = [System.IO.Path]::GetFullPath((Join-Path $guiRoot '..'))

  # Refuse output root inside repository directory (must be outside repository, e.g. temp)
  if ($canonicalOutputRoot.Equals($canonicalRepoRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
      $canonicalOutputRoot.StartsWith($canonicalRepoRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "BuildOutputRoot must be located outside the repository directory: $canonicalOutputRoot"
  }

  # Refuse overlap with default target root (same path or ancestor/descendant)
  if ($canonicalOutputRoot.Equals($canonicalDefaultTarget, [System.StringComparison]::OrdinalIgnoreCase) -or
      $canonicalOutputRoot.StartsWith($canonicalDefaultTarget + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase) -or
      $canonicalDefaultTarget.StartsWith($canonicalOutputRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "BuildOutputRoot must not overlap with default target output root: $canonicalOutputRoot"
  }

  # Refuse reparse points / symlinks on BuildOutputRoot or existing ancestors
  $testDir = $canonicalOutputRoot
  while (-not [string]::IsNullOrWhiteSpace($testDir) -and (Test-Path -LiteralPath $testDir)) {
    $dirItem = Get-Item -LiteralPath $testDir -Force
    if (($dirItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "BuildOutputRoot or an ancestor directory is a reparse point or symbolic link: $testDir"
    }
    $parent = Split-Path -Parent $testDir
    if ($parent -eq $testDir) { break }
    $testDir = $parent
  }

  # Refuse non-empty existing directory
  if (Test-Path -LiteralPath $canonicalOutputRoot) {
    $existingItems = @(Get-ChildItem -LiteralPath $canonicalOutputRoot -Force)
    if ($existingItems.Count -gt 0) {
      throw "BuildOutputRoot exists and is not empty: $canonicalOutputRoot"
    }
  }

  $targetRoot = Join-Path $canonicalOutputRoot 'target'
  $isStagingBuild = $true

  if (-not (Test-Path -LiteralPath $TauriConfigOverlay -PathType Leaf)) {
    throw "TauriConfigOverlay file does not exist: $TauriConfigOverlay"
  }
  $canonicalOverlayPath = [System.IO.Path]::GetFullPath($TauriConfigOverlay)

  # Validate JSON structure, whitelist allowed keys, and verify endpoints
  $overlayJson = $null
  try {
    $overlayJson = Get-Content -LiteralPath $canonicalOverlayPath -Raw | ConvertFrom-Json
  } catch {
    throw "TauriConfigOverlay is not valid JSON: $($_.Exception.Message)"
  }

  # Whitelist validation: only exact lowercase 'plugins.updater' is allowed in overlay to prevent tampering
  $rootProps = @($overlayJson.psobject.Properties | ForEach-Object { $_.Name })
  foreach ($prop in $rootProps) {
    if ($prop -cne 'plugins') {
      throw "TauriConfigOverlay specifies unauthorized top-level key '$prop'. Only 'plugins.updater' is permitted."
    }
  }
  if ($overlayJson.plugins) {
    $pluginsProps = @($overlayJson.plugins.psobject.Properties | ForEach-Object { $_.Name })
    foreach ($prop in $pluginsProps) {
      if ($prop -cne 'updater') {
        throw "TauriConfigOverlay specifies unauthorized key 'plugins.$prop'. Only 'plugins.updater' is permitted."
      }
    }
  }
  if ($overlayJson.plugins.updater) {
    $updaterProps = @($overlayJson.plugins.updater.psobject.Properties | ForEach-Object { $_.Name })
    foreach ($prop in $updaterProps) {
      if ($prop -cne 'endpoints' -and $prop -cne 'pubkey') {
        throw "TauriConfigOverlay specifies unauthorized key 'plugins.updater.$prop'. Only 'endpoints' and 'pubkey' are permitted."
      }
    }
  }

  $endpoints = $overlayJson.plugins.updater.endpoints
  if ($null -eq $endpoints -or $endpoints.Count -eq 0) {
    throw 'TauriConfigOverlay must define at least one updater endpoint in plugins.updater.endpoints.'
  }
  if ($endpoints.Count -ne 1) {
    throw 'Staging overlay must define exactly one isolated updater endpoint.'
  }
  $defaultProdEndpoint = 'https://github.com/soheidon/poverello-coding-tools/releases/latest/download/latest.json'
  foreach ($ep in $endpoints) {
    $epStr = [string]$ep
    if (-not $epStr.StartsWith('https://', [System.StringComparison]::OrdinalIgnoreCase)) {
      throw "TauriConfigOverlay updater endpoint must use HTTPS: $epStr"
    }
    if ($epStr.Equals($defaultProdEndpoint, [System.StringComparison]::OrdinalIgnoreCase)) {
      throw "TauriConfigOverlay must not point to the default production endpoint: $epStr"
    }
    $endpointUri = $null
    if (-not [System.Uri]::TryCreate($epStr, [System.UriKind]::Absolute, [ref]$endpointUri)) {
      throw "Staging endpoint must be a valid absolute HTTPS URL: $epStr"
    }
    if ($endpointUri.UserInfo -or $endpointUri.Query -or $endpointUri.Fragment -or -not $endpointUri.AbsolutePath.EndsWith('/latest.json', [System.StringComparison]::OrdinalIgnoreCase)) {
      throw "Staging endpoint must be a clean HTTPS URL ending in /latest.json: $epStr"
    }
    $isGitHubHost = $endpointUri.Host.Equals('github.com', [System.StringComparison]::OrdinalIgnoreCase) -or
        $endpointUri.Host.EndsWith('.github.com', [System.StringComparison]::OrdinalIgnoreCase) -or
        $endpointUri.Host.Equals('github.io', [System.StringComparison]::OrdinalIgnoreCase) -or
        $endpointUri.Host.EndsWith('.github.io', [System.StringComparison]::OrdinalIgnoreCase) -or
        $endpointUri.Host.EndsWith('.githubusercontent.com', [System.StringComparison]::OrdinalIgnoreCase)
    if ($isGitHubHost) {
      $smokePattern = '^/soheidon/poverello-coding-tools/releases/download/(pct-updater-smoke-[0-9A-Za-z._-]+)/latest\.json$'
      if ($endpointUri.Host.Equals('github.com', [System.StringComparison]::OrdinalIgnoreCase) -and
          $endpointUri.AbsolutePath -cmatch $smokePattern) {
        # Allowed staging smoke prerelease endpoint under INS-007
      } else {
        throw "Staging endpoint must not use GitHub production hosting: $epStr"
      }
    }
  }
  $overlayPublicKey = [string]$overlayJson.plugins.updater.pubkey
  $configuredPublicKey = [Environment]::GetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', 'Process')
  if ([string]::IsNullOrWhiteSpace($overlayPublicKey) -or $overlayPublicKey -cne $configuredPublicKey) {
    throw 'Staging overlay public key must exactly match PCT_UPDATER_PUBLIC_KEY.'
  }
} else {
  $targetRoot = $canonicalDefaultTarget
  $canonicalOverlayPath = $null
}

$nsisRoot = Join-Path $targetRoot 'release\nsis\x64'
$installerScript = Join-Path $nsisRoot 'installer.nsi'
$stagedInstaller = Join-Path $nsisRoot 'nsis-output.exe'
$bundleRoot = Join-Path $targetRoot 'release\bundle\nsis'
$installerName = if ($isStagingBuild) {
  "Poverello-Coding-Tools_${appVersion}_x64-setup-staging.exe"
} else {
  "Poverello-Coding-Tools_${appVersion}_x64-setup.exe"
}
$bundleInstaller = Join-Path $bundleRoot $installerName
$updaterSignature = "$bundleInstaller.sig"
$updaterArtifactsConfig = Join-Path $tauriRoot 'tauri.updater-artifacts.conf.json'

# --- INS-005 Section A: Signing key path resolution ---
$rawPrivateKeyEnv = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'Process')
$rawPrivateKeyPathEnv = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', 'Process')
$rawPasswordEnv = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'Process')

$signingKeyPath = $null
if (-not [string]::IsNullOrWhiteSpace($rawPrivateKeyPathEnv)) {
  $signingKeyPath = $rawPrivateKeyPathEnv
} elseif (-not [string]::IsNullOrWhiteSpace($rawPrivateKeyEnv) -and (Test-Path -LiteralPath $rawPrivateKeyEnv -PathType Leaf)) {
  $signingKeyPath = $rawPrivateKeyEnv
}

$hasSigningPrivateKey = -not [string]::IsNullOrWhiteSpace($signingKeyPath) -and (Test-Path -LiteralPath $signingKeyPath -PathType Leaf)
$hasUpdaterPublicKey = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', 'Process'))

# --- INS-005 Section B: Release candidate fail-closed preflight guard ---
if ($RequireUpdaterSigning) {
  if ($ReuseGeneratedBundle) {
    throw '[-RequireUpdaterSigning] Release candidate build does not permit -ReuseGeneratedBundle. Fresh stable build is required.'
  }

  if (-not ($env:OS -match 'Windows')) {
    throw "[$BuildMode] updater-signed NSIS builds require Windows OS."
  }

  $channel = [Environment]::GetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'Process')
  if ($channel -ne $expectedChannel) {
    throw "[$BuildMode] build requires ANTHRO_BRIDGE_CHANNEL='$expectedChannel', but got '$channel'."
  }

  if (-not $hasUpdaterPublicKey) {
    throw "[$BuildMode] build requires PCT_UPDATER_PUBLIC_KEY."
  }

  if (-not $hasSigningPrivateKey -or [string]::IsNullOrWhiteSpace($signingKeyPath)) {
    throw "[$BuildMode] build requires an accessible private key file path."
  }
  if (-not (Test-Path -LiteralPath $signingKeyPath -PathType Leaf)) {
    throw "[$BuildMode] private key file does not exist or is not a leaf file: $signingKeyPath"
  }
  try {
    $testStream = [System.IO.File]::OpenRead($signingKeyPath)
    $testStream.Dispose()
  } catch {
    throw "[$BuildMode] private key file is not readable: $signingKeyPath"
  }

  if ([string]::IsNullOrWhiteSpace($rawPasswordEnv)) {
    throw "[$BuildMode] build requires TAURI_SIGNING_PRIVATE_KEY_PASSWORD environment variable."
  }

  if (-not (Test-Path -LiteralPath $updaterArtifactsConfig -PathType Leaf)) {
    throw "[$BuildMode] build requires updater artifact configuration: $updaterArtifactsConfig"
  }
  try {
    $artifactSettings = Get-Content -LiteralPath $updaterArtifactsConfig -Raw | ConvertFrom-Json
  } catch {
    throw "[$BuildMode] updater artifact configuration is invalid JSON: $updaterArtifactsConfig"
  }
  if ($artifactSettings.bundle.createUpdaterArtifacts -ne $true) {
    throw "[$BuildMode] updater artifact signing must be enabled in $updaterArtifactsConfig"
  }

  if ($BuildMode -eq 'Stable') {
    $stableEndpoint = 'https://github.com/soheidon/poverello-coding-tools/releases/latest/download/latest.json'
    $configuredEndpoints = @($appConfig.plugins.updater.endpoints)
    if ($configuredEndpoints.Count -ne 1 -or [string]$configuredEndpoints[0] -cne $stableEndpoint) {
      throw "[Stable] tauri.conf.json must contain exactly the approved production endpoint: $stableEndpoint"
    }
  }

  Write-Output "[$BuildMode] updater signing preflight passed: Windows, matching channel/mode, public key set, private key path readable, password configured, artifact config present."
}

if ($hasSigningPrivateKey -and -not $hasUpdaterPublicKey) {
  throw 'Updater signing is configured without PCT_UPDATER_PUBLIC_KEY; refusing to produce an untrusted signed update.'
}
if ($hasSigningPrivateKey -and -not (Test-Path -LiteralPath $updaterArtifactsConfig -PathType Leaf)) {
  throw "Updater artifact configuration is missing: $updaterArtifactsConfig"
}
if ($hasUpdaterPublicKey) {
  $publicKeyVerifier = Join-Path $PSScriptRoot 'verify-updater-signature.mjs'
  $keyValidation = & node $publicKeyVerifier --validate-public-key-env 2>&1
  $keyValidationExitCode = $LASTEXITCODE
  $keyValidation | ForEach-Object { Write-Output $_ }
  if ($keyValidationExitCode -ne 0) {
    throw 'PCT_UPDATER_PUBLIC_KEY is not valid base64-encoded Tauri Minisign public-key text.'
  }
}

if ($PreflightOnly) {
  Write-Output '[-PreflightOnly] Updater signing preflight checks passed successfully. Exiting without initiating build.'
  exit 0
}

$makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
$buildLogRoot = Join-Path $targetRoot 'release\build-logs'
New-Item -ItemType Directory -Path $buildLogRoot -Force | Out-Null
$script:buildLogPath = Join-Path $buildLogRoot ("nsis-{0}-{1}.log" -f (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'), [guid]::NewGuid().ToString('N'))
$buildStartedUtc = [DateTime]::UtcNow
$sourceHead = (& git -C $guiRoot\.. rev-parse HEAD 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not capture source HEAD for build provenance.' }
$dirtyStatus = (& git -C $guiRoot\.. status --porcelain --untracked-files=all 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not capture dirty-worktree state for build provenance.' }
$exePath = Join-Path $targetRoot 'release\anthro-bridge.exe'
$preBuildExeInfo = if (Test-Path -LiteralPath $exePath -PathType Leaf) { Get-Item -LiteralPath $exePath } else { $null }
$preBuildExeHash = if ($preBuildExeInfo) { Get-FileSha256Hex -Path $exePath } else { '<missing>' }
Start-Transcript -LiteralPath $script:buildLogPath | Out-Null
$script:transcriptStarted = $true
Write-Output "Build log: $script:buildLogPath"
Write-Output "Source HEAD: $sourceHead"
Write-Output "Dirty worktree: $([bool]$dirtyStatus)"
Write-Output "Build started UTC: $($buildStartedUtc.ToString('o'))"
Write-Output "Staging build: $isStagingBuild"
Write-Output "Target root: $targetRoot"
Write-Output "RequireUpdaterSigning: $RequireUpdaterSigning"
Write-Output "Updater client configured: $hasUpdaterPublicKey"
Write-Output "Updater artifact signing enabled: $hasSigningPrivateKey"
Write-Output "Signing key path configured: $(-not [string]::IsNullOrWhiteSpace($signingKeyPath))"
Write-Output 'Effective CARGO_PROFILE_RELEASE_CODEGEN_UNITS: 1'
Write-Output "Pre-build application path: $exePath"
Write-Output "Pre-build application SHA-256: $preBuildExeHash"
Write-Output "Pre-build application bytes: $(if ($preBuildExeInfo) { $preBuildExeInfo.Length } else { '<missing>' })"
Write-Output "Pre-build application last-write UTC: $(if ($preBuildExeInfo) { $preBuildExeInfo.LastWriteTimeUtc.ToString('o') } else { '<missing>' })"

if (-not $ReuseGeneratedBundle) {
  $previousCodegenUnits = [Environment]::GetEnvironmentVariable('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', 'Process')
  $previousCargoTarget = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')
  Push-Location $guiRoot
  try {
    [Environment]::SetEnvironmentVariable('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', '1', 'Process')
    if ($isStagingBuild) {
      [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $targetRoot, 'Process')
    }
    $previousErrorActionPreference = $ErrorActionPreference
    try {
      # Windows PowerShell 5.1 can turn ordinary native stderr (including
      # Tauri's informational lines) into terminating PowerShell errors under
      # ErrorActionPreference=Stop. Capture the stream and classify the actual
      # process result explicitly below instead of aborting on stream routing.
      $ErrorActionPreference = 'Continue'
      $rawBuildScript = if ($BuildMode -eq 'Stable') { 'tauri:build:stable:raw' } else { 'tauri:build:staging:raw' }
      $tauriArgs = @('run', $rawBuildScript, '--', '--bundles', 'nsis')
      if ($hasSigningPrivateKey) {
        $tauriArgs += @('--config', 'src-tauri/tauri.updater-artifacts.conf.json')
      }
      if ($canonicalOverlayPath) {
        $tauriArgs += @('--config', $canonicalOverlayPath)
      }
      $tauriOutput = & npm $tauriArgs 2>&1
      $tauriExitCode = $LASTEXITCODE
    }
    finally {
      $ErrorActionPreference = $previousErrorActionPreference
    }
    $tauriOutput | ForEach-Object { Write-Output $_ }
    if ($tauriExitCode -ne 0) {
      $tauriText = ($tauriOutput | ForEach-Object { $_.ToString() }) -join "`n"
      $errorLines = @($tauriText -split "`r?`n" | Where-Object { $_ -match '(?i)\berror\s*:' })
      $scriptFresh = (Test-Path -LiteralPath $installerScript -PathType Leaf) -and ((Get-Item -LiteralPath $installerScript).LastWriteTimeUtc -ge $buildStartedUtc.AddSeconds(-2))
      $exeFreshOrCargoFresh = (Test-Path -LiteralPath $exePath -PathType Leaf) -and ($tauriText -match '(?i)Finished\s+`?release`?\s+profile')
      $expectedInstallerHookExpansionFailure = $tauriText.Contains('resolving install function "PCTValidatePoverelloRegistration"')
      $expectedUninstallerHookExpansionFailure =
        $tauriText.Contains('resolving uninstall function "un.PCTDeleteStableUserData"') -or
        $tauriText.Contains('resolving uninstall function "un.PCTSnapshotUpdateRegistration"')
      $expectedHookExpansionFailure =
        ($errorLines.Count -eq 1) -and
        ($expectedInstallerHookExpansionFailure -or $expectedUninstallerHookExpansionFailure) -and
        $tauriText.Contains('failed to bundle project') -and
        $scriptFresh -and $exeFreshOrCargoFresh
      if (-not $expectedHookExpansionFailure) {
        throw "Tauri stable NSIS build failed with exit code $tauriExitCode outside the verified PCT hook-expansion boundary."
      }
      Write-Warning 'Tauri frontend/Rust release build completed, and this invocation generated the NSIS template, but its preliminary makensis pass cannot resolve the PCT hook function before post-generation expansion. Continuing only because the release profile completed and the freshly generated template and versioned payload are present; patched makensis compilation remains mandatory.'
    }
  }
  finally {
    Pop-Location
    [Environment]::SetEnvironmentVariable('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', $previousCodegenUnits, 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $previousCargoTarget, 'Process')
  }
}

if (-not (Test-Path -LiteralPath $installerScript -PathType Leaf)) {
  throw "Tauri did not generate the expected installer template: $installerScript"
}
if (-not $ReuseGeneratedBundle -and (Get-Item -LiteralPath $installerScript).LastWriteTimeUtc -lt $buildStartedUtc.AddSeconds(-2)) {
  throw 'The generated NSIS installer script is not fresh for this invocation.'
}
if (-not (Test-Path -LiteralPath $makensis -PathType Leaf)) {
  throw "The configured NSIS compiler was not found: $makensis"
}
$payloadInfo = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)
if ($payloadInfo.ProductName -ne $productName -or $payloadInfo.ProductVersion -ne $appVersion) {
  throw "Release application payload metadata mismatch: ProductName='$($payloadInfo.ProductName)', ProductVersion='$($payloadInfo.ProductVersion)'"
}
$script:HOMEPAGECheck = [regex]::Match([System.IO.File]::ReadAllText($installerScript, [System.Text.Encoding]::UTF8), '(?m)^!define HOMEPAGE "([^"]*)"')
if (-not $script:HOMEPAGECheck.Success -or $script:HOMEPAGECheck.Groups[1].Value -ne '') {
  throw 'Current generated NSIS payload writes optional URL registration values; extend snapshot/restore verification before allowing this build.'
}
$script:installModeCheck = [regex]::Match([System.IO.File]::ReadAllText($installerScript, [System.Text.Encoding]::UTF8), '(?m)^!define INSTALLMODE "([^"]*)"')
if (-not $script:installModeCheck.Success -or $script:installModeCheck.Groups[1].Value -ne 'currentUser') {
  throw 'The recovery snapshot contract is scoped to current-user registration; extend it before building any other NSIS install mode.'
}

# Patch only the generated build artifact. Exact-anchor and count checks make a
# Tauri template change fail closed instead of silently shipping an unpatched
# installer. Keep the generated file's UTF-8 BOM and CRLF format.
$scriptText = [System.IO.File]::ReadAllText($installerScript, [System.Text.Encoding]::UTF8)
if ($ReuseGeneratedBundle) {
  $expectedExe = Join-Path $targetRoot 'release\anthro-bridge.exe'
  if (-not (Test-Path -LiteralPath $expectedExe -PathType Leaf)) {
    throw "Cannot reuse generated NSIS bundle: application binary is missing: $expectedExe"
  }
  $expectedScriptMetadata = @(
    '!define PRODUCTNAME "Poverello Coding Tools"',
    "!define VERSION `"$appVersion`"",
    '!define ARCH "x64"',
    '!define DISPLAYLANGUAGESELECTOR "true"',
    "!define MAINBINARYSRCPATH `"$expectedExe`""
  )
  foreach ($metadata in $expectedScriptMetadata) {
    if (-not $scriptText.Contains($metadata)) {
      throw "Refusing NSIS-only rebuild: generated script does not match current v$appVersion x64 configuration: $metadata"
    }
  }
  $exeInfo = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($expectedExe)
  if ($exeInfo.ProductName -ne $productName -or $exeInfo.ProductVersion -ne $appVersion) {
    throw "Refusing NSIS-only rebuild: application payload metadata is '$($exeInfo.ProductName)' '$($exeInfo.ProductVersion)', expected '$productName' '$appVersion'."
  }
}
$onInitAnchor = "  !insertmacro SetContext`r`n"
$onInitStart = $scriptText.IndexOf("Function .onInit`r`n", [System.StringComparison]::Ordinal)
$onInitEnd = if ($onInitStart -ge 0) { $scriptText.IndexOf("FunctionEnd`r`n", $onInitStart, [System.StringComparison]::Ordinal) } else { -1 }
if ($onInitStart -lt 0 -or $onInitEnd -lt 0) {
  throw 'Could not locate the complete Tauri .onInit function. No patched installer was produced.'
}
$onInitBody = $scriptText.Substring($onInitStart, $onInitEnd - $onInitStart)
$onInitMatches = [regex]::Matches($onInitBody, [regex]::Escape($onInitAnchor))
if ($onInitMatches.Count -ne 1) {
  throw "Expected exactly one .onInit SetContext anchor, found $($onInitMatches.Count). No patched installer was produced."
}

# hooks.nsh is included before PRODUCTNAME/UNINSTKEY/MAINBINARYNAME and the
# installer variables are defined. Expand its functions only after both sets
# of declarations are available.
$functionMacroAnchor = 'Var OldMainBinaryName' + "`r`n"
$functionMacroMarker = '!insertmacro PCT_HOOK_FUNCTIONS'
$existingFunctionMacro = [regex]::Matches($scriptText, '(?m)^!insertmacro PCT_HOOK_FUNCTIONS\r?\n')
if ($existingFunctionMacro.Count -gt 1) {
  throw 'Generated NSIS script contains multiple PCT hook-function expansions; refusing ambiguous patch.'
}
if ($existingFunctionMacro.Count -eq 1) {
  $scriptText = [regex]::Replace($scriptText, '(?m)^!insertmacro PCT_HOOK_FUNCTIONS\r?\n', '', 1)
}
$functionMacroMatches = [regex]::Matches($scriptText, [regex]::Escape($functionMacroAnchor))
if ($functionMacroMatches.Count -ne 1) {
  throw "Expected exactly one post-metadata installer-variable anchor, found $($functionMacroMatches.Count)."
}
$scriptText = $scriptText.Replace($functionMacroAnchor, $functionMacroAnchor + $functionMacroMarker + "`r`n")

$oldBridgePattern = '(?s)  ; Poverello 0\.24\.1 upgrade bridge.*?  StrCpy \$UpdateMode 1\r?\n  \$\{EndIf\}\r?\n'
$oldBridgeMatches = [regex]::Matches($scriptText, $oldBridgePattern)
if ($oldBridgeMatches.Count -gt 1) { throw "Found multiple legacy bridge blocks ($($oldBridgeMatches.Count)); refusing ambiguous patch." }
if ($oldBridgeMatches.Count -eq 1) {
  $scriptText = [regex]::Replace($scriptText, $oldBridgePattern, "  Call PCTClassifyInstallState`r`n", 1)
} else {
  $onInitStart = $scriptText.IndexOf("Function .onInit`r`n", [System.StringComparison]::Ordinal)
  $onInitEnd = if ($onInitStart -ge 0) { $scriptText.IndexOf("FunctionEnd`r`n", $onInitStart, [System.StringComparison]::Ordinal) } else { -1 }
  if ($onInitStart -lt 0 -or $onInitEnd -lt 0) { throw 'Could not locate the complete Tauri .onInit function.' }
  $onInitBody = $scriptText.Substring($onInitStart, $onInitEnd - $onInitStart)
  $onInitMatches = [regex]::Matches($onInitBody, [regex]::Escape($onInitAnchor))
  if ($onInitMatches.Count -ne 1) { throw "Expected exactly one .onInit SetContext anchor, found $($onInitMatches.Count)." }
  $classifyCalls = [regex]::Matches($onInitBody, '(?m)^\s*Call PCTClassifyInstallState\s*$')
  if ($classifyCalls.Count -gt 1) { throw 'Generated .onInit contains multiple state-classification calls.' }
  if ($classifyCalls.Count -eq 0) {
    $onInitBody = $onInitBody.Replace($onInitAnchor, $onInitAnchor + "  Call PCTClassifyInstallState`r`n")
  }
  $scriptText = $scriptText.Substring(0, $onInitStart) + $onInitBody + $scriptText.Substring($onInitEnd)
}

$directoryPagePattern = '(?m)^!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive\r?\n(?:!define MUI_PAGE_CUSTOMFUNCTION_LEAVE [^\r\n]+\r?\n)?!insertmacro MUI_PAGE_DIRECTORY\r?\n'
$directoryPageMatches = [regex]::Matches($scriptText, $directoryPagePattern)
if ($directoryPageMatches.Count -ne 1) {
  throw "Expected exactly one Tauri directory-page anchor, found $($directoryPageMatches.Count). No patched installer was produced."
}
$directoryPagePatched = "!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive`r`n!define MUI_PAGE_CUSTOMFUNCTION_LEAVE ValidateLegacyUpgradeDir`r`n!insertmacro MUI_PAGE_DIRECTORY`r`n"
$scriptText = [regex]::Replace($scriptText, $directoryPagePattern, [System.Text.RegularExpressions.MatchEvaluator]{ param($match) $directoryPagePatched }, 1)

# Tauri normally mutates Start Menu/Desktop shortcuts before the postinstall
# registration check. Guard those production calls, then invoke the same
# routines from NSIS_HOOK_POSTINSTALL after Poverello registration is verified.
$installSectionPattern = '(?s)(Section Install\b)(.*?)(SectionEnd)'
$installSectionMatches = [regex]::Matches($scriptText, $installSectionPattern)
if ($installSectionMatches.Count -ne 1) { throw "Expected exactly one generated Install section, found $($installSectionMatches.Count)." }
$installBody = $installSectionMatches[0].Groups[2].Value
foreach ($shortcut in @('CreateOrUpdateStartMenuShortcut', 'CreateOrUpdateDesktopShortcut')) {
  $callPattern = "(?m)^(\s*)Call $shortcut\s*$"
  $callMatches = [regex]::Matches($installBody, $callPattern)
  if ($callMatches.Count -ne 1) { throw "Expected exactly one pre-verification Install call to $shortcut, found $($callMatches.Count)." }
  $installBody = [regex]::Replace($installBody, $callPattern, [System.Text.RegularExpressions.MatchEvaluator]{
    param($match)
    $indent = $match.Groups[1].Value
    return "${indent}`${If} `$PCTRegistrationVerified == 1`r`n${indent}  Call $shortcut`r`n${indent}`${EndIf}"
  }, 1)
}
$installSectionMatches = [regex]::Matches($scriptText, $installSectionPattern)
$installMatch = $installSectionMatches[0]
$scriptText = $scriptText.Substring(0, $installMatch.Groups[2].Index) + $installBody + $scriptText.Substring($installMatch.Groups[2].Index + $installMatch.Groups[2].Length)

# Older build-helper versions injected callback bodies into installer.nsi. The
# authoritative functions now live in hooks.nsh; remove only those exact
# generated definitions so the included callbacks are not duplicated.
foreach ($functionName in @('.onGUIEnd', 'ValidateLegacyUpgradeDir')) {
  $escapedName = [regex]::Escape($functionName)
  $functionPattern = "(?s)Function $escapedName\r?\n.*?FunctionEnd\r?\n"
  $functionMatches = [regex]::Matches($scriptText, $functionPattern)
  if ($functionMatches.Count -gt 1) { throw "Found multiple generated $functionName functions; refusing ambiguous patch." }
  if ($functionMatches.Count -eq 1) {
    $scriptText = [regex]::Replace($scriptText, $functionPattern, '', 1)
  }
}

$hookPath = Join-Path $tauriRoot 'windows\nsis\hooks.nsh'
$hookText = [System.IO.File]::ReadAllText($hookPath)
$verifyNeedles = @(
  'Call PCTClassifyInstallState',
  'ValidateLegacyUpgradeDir',
  'Function .onGUIEnd',
  'Function PCTValidateLegacyRegistration',
  'Function PCTValidatePoverelloRegistration',
  'InstallLocation',
  'UninstallString',
  'DisplayIcon',
  'LegacyUpgradePending',
  'SetErrorLevel 1',
  'PCTRestoreRegistrationOnFailure',
  'StrCpy $PCTRegistrationVerified 1'
)
foreach ($needle in $verifyNeedles) {
  if (-not $scriptText.Contains($needle) -and -not $hookText.Contains($needle)) { throw "Patched installer/hooks are missing required migration logic: $needle" }
}
if (-not $scriptText.Contains('!define DISPLAYLANGUAGESELECTOR "true"') -or
    -not $scriptText.Contains('!define ARCH "x64"')) {
  throw 'The generated NSIS script no longer matches the approved x64/multilingual release configuration.'
}
foreach ($language in @('English', 'Japanese', 'SimpChinese', 'TradChinese', 'Korean', 'French', 'German', 'Spanish')) {
  if (-not $scriptText.Contains("$language.nsh")) { throw "Expected NSIS language file is missing: $language" }
}
if (-not $scriptText.Contains('MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"') -or
    -not $scriptText.Contains('MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"') -or
    -not $scriptText.Contains('icons\header_image.bmp') -or
    -not (Test-Path -LiteralPath (Join-Path $tauriRoot 'icons\header_image.bmp') -PathType Leaf)) {
  throw 'Poverello installer and uninstaller header artwork is not wired to the configured BMP.'
}
if (-not $scriptText.Contains($functionMacroMarker) -or $scriptText.Contains('Function PCTClassifyInstallState')) {
  throw 'The PCT hook function macro is missing or was expanded more than once.'
}

[System.IO.File]::WriteAllText($installerScript, $scriptText, [System.Text.UTF8Encoding]::new($true))
if (Test-Path -LiteralPath $stagedInstaller) {
  Remove-Item -LiteralPath $stagedInstaller -Force
}
Push-Location $nsisRoot
try {
  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = 'Continue'
    $compileOutput = & $makensis /V2 installer.nsi 2>&1
    $compileExitCode = $LASTEXITCODE
  }
  finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  $compileOutput | ForEach-Object { Write-Output $_ }
  if ($compileExitCode -ne 0) { throw "Patched NSIS compilation failed with exit code $compileExitCode" }
}
finally {
  Pop-Location
}

if (-not (Test-Path -LiteralPath $stagedInstaller -PathType Leaf)) {
  throw "NSIS did not produce the expected staged installer: $stagedInstaller"
}
New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null
Copy-Item -LiteralPath $stagedInstaller -Destination $bundleInstaller -Force

$file = Get-Item -LiteralPath $bundleInstaller
$sha256 = [System.Security.Cryptography.SHA256]::Create()
$fileStream = [System.IO.File]::OpenRead($bundleInstaller)
try {
  $hash = [System.BitConverter]::ToString($sha256.ComputeHash($fileStream)).Replace('-', '')
}
finally {
  $fileStream.Dispose()
  $sha256.Dispose()
}
$version = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($bundleInstaller)
if ($version.ProductName -ne $productName -or $version.ProductVersion -ne $appVersion) {
  throw "Final installer metadata mismatch: ProductName='$($version.ProductName)', ProductVersion='$($version.ProductVersion)'"
}

if (-not $hasSigningPrivateKey -and (Test-Path -LiteralPath $updaterSignature)) {
  Remove-Item -LiteralPath $updaterSignature -Force
  Write-Output 'Removed a stale updater signature because this build is not signed.'
}

if ($hasSigningPrivateKey) {
  $tauriCli = Join-Path $guiRoot 'node_modules\.bin\tauri.cmd'
  if (-not (Test-Path -LiteralPath $tauriCli -PathType Leaf)) {
    throw "Tauri CLI was not found for final updater signing: $tauriCli"
  }
  if (Test-Path -LiteralPath $updaterSignature) {
    Remove-Item -LiteralPath $updaterSignature -Force
  }
  Push-Location $guiRoot
  $origPrivateKeyEnv = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'Process')
  $origPrivateKeyPathEnv = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', 'Process')
  try {
    # Hand off private key path to TAURI_SIGNING_PRIVATE_KEY_PATH for Tauri CLI signer sign
    # and temporarily clear TAURI_SIGNING_PRIVATE_KEY so the path string is not misinterpreted as raw key text
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
    Remove-Item env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $signingKeyPath, 'Process')
    $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $signingKeyPath
    $signOutput = & $tauriCli signer sign --app-version $appVersion $bundleInstaller 2>&1
    $signExitCode = $LASTEXITCODE
    $signOutput | ForEach-Object { Write-Output $_ }
  }
  finally {
    Pop-Location
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $origPrivateKeyEnv, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $origPrivateKeyPathEnv, 'Process')
  }
  if ($signExitCode -ne 0 -or -not (Test-Path -LiteralPath $updaterSignature -PathType Leaf)) {
    throw "Tauri Updater signing of the final patched NSIS installer failed (exit code $signExitCode)."
  }
  $signatureVerifier = Join-Path $PSScriptRoot 'verify-updater-signature.mjs'
  if (-not (Test-Path -LiteralPath $signatureVerifier -PathType Leaf)) {
    throw "Tauri-compatible offline updater signature verifier was not found: $signatureVerifier"
  }
  $verifyOutput = & node $signatureVerifier $bundleInstaller $updaterSignature '-' $appVersion 2>&1
  $verifyExitCode = $LASTEXITCODE
  $verifyOutput | ForEach-Object { Write-Output $_ }
  if ($verifyExitCode -ne 0) {
    throw "Final patched NSIS updater signature verification failed (exit code $verifyExitCode)."
  }

  $manifestGenerator = Join-Path $PSScriptRoot 'generate-updater-manifest.mjs'
  if (-not (Test-Path -LiteralPath $manifestGenerator -PathType Leaf)) {
    throw "Updater manifest generator was not found: $manifestGenerator"
  }
  $manifestPath = Join-Path $bundleRoot 'latest.json'
  if ($BuildMode -eq 'Stable') {
    $assetUrl = "https://github.com/soheidon/poverello-coding-tools/releases/download/v$appVersion/$installerName"
  } else {
    $stagingEndpoint = [string]$endpoints[0]
    $stagingUri = [System.Uri]$stagingEndpoint
    $endpointPrefix = $stagingUri.GetLeftPart([System.UriPartial]::Authority) + $stagingUri.AbsolutePath.Substring(0, $stagingUri.AbsolutePath.LastIndexOf('/') + 1)
    $assetUrl = "$endpointPrefix$installerName"
  }
  $manifestArgs = @(
    $manifestGenerator,
    '--mode', $BuildMode.ToLowerInvariant(),
    '--version', $appVersion,
    '--installer', $bundleInstaller,
    '--signature', $updaterSignature,
    '--sha256', $hash,
    '--news', (Join-Path (Split-Path -Parent $guiRoot) 'NEWS.md'),
    '--asset-url', $assetUrl,
    '--output', $manifestPath
  )
  if ($BuildMode -eq 'Staging') { $manifestArgs += @('--staging-endpoint', $stagingEndpoint) }
  $manifestOutput = & node @manifestArgs 2>&1
  $manifestExitCode = $LASTEXITCODE
  $manifestOutput | ForEach-Object { Write-Output $_ }
  if ($manifestExitCode -ne 0 -or -not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Updater manifest generation failed (exit code $manifestExitCode)."
  }
  $manifestVerifier = Join-Path $PSScriptRoot 'verify-updater-manifest.mjs'
  if (-not (Test-Path -LiteralPath $manifestVerifier -PathType Leaf)) {
    throw "Updater manifest verifier was not found: $manifestVerifier"
  }
  $manifestVerifierArgs = @($manifestVerifier, $manifestPath, $BuildMode.ToLowerInvariant(), $appVersion, $assetUrl)
  if ($BuildMode -eq 'Staging') { $manifestVerifierArgs += $stagingEndpoint }
  $manifestValidation = & node @manifestVerifierArgs 2>&1
  $manifestValidationExitCode = $LASTEXITCODE
  if ($manifestValidationExitCode -ne 0) {
    throw 'Generated updater manifest does not match the signed installer version, URL, or signature.'
  }
  Write-Output "Updater manifest: $manifestPath"
  Write-Output "Final updater signature: $updaterSignature"
  Write-Output "Final updater signature SHA-256: $(Get-FileSha256Hex -Path $updaterSignature)"
}
elseif ($hasUpdaterPublicKey) {
  Write-Warning 'Updater client public key is configured, but no user-managed signing key is available. The installer can check updates after release, but this build has no signed updater artifact.'
}
else {
  Write-Warning 'Updater is disabled in this build because PCT_UPDATER_PUBLIC_KEY is not configured.'
}

Write-Output "Patched release installer: $($file.FullName)"
Write-Output "Bytes: $($file.Length)"
Write-Output "SHA-256: $hash"
Write-Output "Product: $($version.ProductName) $($version.ProductVersion)"
Write-Output "Expected release basename: $installerName"
$authenticodeStatus = $null
try {
  $authenticodeStatus = [string](Get-AuthenticodeSignature -FilePath $bundleInstaller -ErrorAction Stop).Status
}
catch {
  $statusReason = ([string]$_.Exception.Message -replace '[\r\n\t]+', ' ').Trim()
  if ([string]::IsNullOrWhiteSpace($statusReason)) {
    $statusReason = [string]$_.FullyQualifiedErrorId
  }
  $authenticodeStatus = "Unavailable ($statusReason)"
}
Write-Output "Authenticode status: $authenticodeStatus"
Write-Output "Build finished UTC: $([DateTime]::UtcNow.ToString('o'))"
Write-Output "Application payload last-write UTC: $((Get-Item -LiteralPath $exePath).LastWriteTimeUtc.ToString('o'))"
Write-Output "Application payload SHA-256: $(Get-FileSha256Hex -Path $exePath)"
Write-Output "Application payload metadata: $((([System.Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)).ProductName)) $((([System.Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)).ProductVersion))"
Write-Output "Generated NSIS script SHA-256: $(Get-FileSha256Hex -Path $installerScript)"
Write-Output "Hook source SHA-256: $(Get-FileSha256Hex -Path $hookPath)"
Write-Output "Build helper SHA-256: $(Get-FileSha256Hex -Path $PSCommandPath)"
Write-Output "Pre-build application SHA-256: $preBuildExeHash"
Write-Output "Tauri exit code: $(if ($ReuseGeneratedBundle) { 'not invoked (ReuseGeneratedBundle)' } else { $tauriExitCode })"
Write-Output "Patched NSIS compiler exit code: $compileExitCode"
Write-Output "Final installer SHA-256: $hash"
Write-Output "Final installer bytes: $($file.Length)"
Write-Output "Final installer metadata: $($version.ProductName) $($version.ProductVersion)"
Write-Output "Build log path: $script:buildLogPath"
Stop-Transcript | Out-Null
$script:transcriptStarted = $false
