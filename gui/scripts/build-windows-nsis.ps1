param(
  [switch]$ReuseGeneratedBundle
)

$ErrorActionPreference = 'Stop'
$script:buildLogPath = $null
$script:transcriptStarted = $false
trap {
  if ($script:transcriptStarted) {
    try { Stop-Transcript | Out-Null } catch { }
    $script:transcriptStarted = $false
  }
  throw
}

$guiRoot = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $guiRoot 'src-tauri'
$appConfig = Get-Content -LiteralPath (Join-Path $tauriRoot 'tauri.conf.json') -Raw | ConvertFrom-Json
$productName = [string]$appConfig.productName
$appVersion = [string]$appConfig.version
if ($productName -ne 'Poverello Coding Tools' -or [string]::IsNullOrWhiteSpace($appVersion)) {
  throw "Unexpected stable product metadata: product='$productName', version='$appVersion'"
}
$nsisRoot = Join-Path $tauriRoot 'target\release\nsis\x64'
$installerScript = Join-Path $nsisRoot 'installer.nsi'
$stagedInstaller = Join-Path $nsisRoot 'nsis-output.exe'
$bundleRoot = Join-Path $tauriRoot 'target\release\bundle\nsis'
$installerName = "Poverello-Coding-Tools_${appVersion}_x64-setup.exe"
$bundleInstaller = Join-Path $bundleRoot $installerName
$makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
$buildLogRoot = Join-Path $tauriRoot 'target\release\build-logs'
New-Item -ItemType Directory -Path $buildLogRoot -Force | Out-Null
$script:buildLogPath = Join-Path $buildLogRoot ("nsis-{0}-{1}.log" -f (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'), [guid]::NewGuid().ToString('N'))
$buildStartedUtc = [DateTime]::UtcNow
$sourceHead = (& git -C $guiRoot\.. rev-parse HEAD 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not capture source HEAD for build provenance.' }
$dirtyStatus = (& git -C $guiRoot\.. status --porcelain --untracked-files=all 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not capture dirty-worktree state for build provenance.' }
$exePath = Join-Path $tauriRoot 'target\release\anthro-bridge.exe'
$preBuildExeHash = if (Test-Path -LiteralPath $exePath -PathType Leaf) { (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash } else { '<missing>' }
Start-Transcript -LiteralPath $script:buildLogPath | Out-Null
$script:transcriptStarted = $true
Write-Output "Build log: $script:buildLogPath"
Write-Output "Source HEAD: $sourceHead"
Write-Output "Dirty worktree: $([bool]$dirtyStatus)"
Write-Output "Build started UTC: $($buildStartedUtc.ToString('o'))"
Write-Output "Build command: npm run tauri:build:stable:raw -- --bundles nsis"
Write-Output 'Effective CARGO_PROFILE_RELEASE_CODEGEN_UNITS: 1'
Write-Output "Pre-build application SHA-256: $preBuildExeHash"

if (-not $ReuseGeneratedBundle) {
  $previousCodegenUnits = [Environment]::GetEnvironmentVariable('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', 'Process')
  Push-Location $guiRoot
  try {
    [Environment]::SetEnvironmentVariable('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', '1', 'Process')
    $previousErrorActionPreference = $ErrorActionPreference
    try {
      # Windows PowerShell 5.1 can turn ordinary native stderr (including
      # Tauri's informational lines) into terminating PowerShell errors under
      # ErrorActionPreference=Stop. Capture the stream and classify the actual
      # process result explicitly below instead of aborting on stream routing.
      $ErrorActionPreference = 'Continue'
      $tauriOutput = & npm run tauri:build:stable:raw -- --bundles nsis 2>&1
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
  $expectedExe = Join-Path $tauriRoot 'target\release\anthro-bridge.exe'
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

Write-Output "Patched release installer: $($file.FullName)"
Write-Output "Bytes: $($file.Length)"
Write-Output "SHA-256: $hash"
Write-Output "Product: $($version.ProductName) $($version.ProductVersion)"
Write-Output "Expected release basename: $installerName"
Write-Output "Authenticode status: $((Get-AuthenticodeSignature -FilePath $bundleInstaller).Status)"
Write-Output "Build finished UTC: $([DateTime]::UtcNow.ToString('o'))"
Write-Output "Application payload last-write UTC: $((Get-Item -LiteralPath $exePath).LastWriteTimeUtc.ToString('o'))"
Write-Output "Application payload SHA-256: $((Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash)"
Write-Output "Application payload metadata: $((([System.Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)).ProductName)) $((([System.Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)).ProductVersion))"
Write-Output "Generated NSIS script SHA-256: $((Get-FileHash -LiteralPath $installerScript -Algorithm SHA256).Hash)"
Write-Output "Hook source SHA-256: $((Get-FileHash -LiteralPath $hookPath -Algorithm SHA256).Hash)"
Write-Output "Build helper SHA-256: $((Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash)"
Write-Output "Pre-build application SHA-256: $preBuildExeHash"
Write-Output "Tauri exit code: $(if ($ReuseGeneratedBundle) { 'not invoked (ReuseGeneratedBundle)' } else { $tauriExitCode })"
Write-Output "Patched NSIS compiler exit code: $compileExitCode"
Write-Output "Final installer SHA-256: $hash"
Write-Output "Final installer bytes: $($file.Length)"
Write-Output "Final installer metadata: $($version.ProductName) $($version.ProductVersion)"
Write-Output "Build log path: $script:buildLogPath"
Stop-Transcript | Out-Null
$script:transcriptStarted = $false
