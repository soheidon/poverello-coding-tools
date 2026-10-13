param(
  [string]$InstallerScript = (Join-Path $PSScriptRoot '..\src-tauri\target\release\nsis\x64\installer.nsi'),
  [string]$HooksPath = (Join-Path $PSScriptRoot '..\src-tauri\windows\nsis\hooks.nsh')
)

$ErrorActionPreference = 'Stop'

foreach ($path in @($InstallerScript, $HooksPath)) {
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
    throw "Required NSIS input is missing: $path"
  }
}

$script = [System.IO.File]::ReadAllText((Resolve-Path -LiteralPath $InstallerScript))
$hooks = [System.IO.File]::ReadAllText((Resolve-Path -LiteralPath $HooksPath))

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

function Get-FunctionBody([string]$Text, [string]$Name) {
  $escaped = [regex]::Escape($Name)
  $matches = [regex]::Matches($Text, "(?s)Function $escaped\r?\n(.*?)FunctionEnd")
  Assert-True ($matches.Count -eq 1) "Expected one NSIS function '$Name', found $($matches.Count)."
  return $matches[0].Groups[1].Value
}

$onInit = Get-FunctionBody $script '.onInit'
$setContext = $onInit.IndexOf('!insertmacro SetContext', [System.StringComparison]::Ordinal)
$classify = $onInit.IndexOf('Call PCTClassifyInstallState', [System.StringComparison]::Ordinal)
$firstRegistryWrite = [regex]::Match($onInit, '(?m)^\s*(WriteReg(?:Str|DWORD|Bin|ExpandStr|MultiStr)|DeleteReg(?:Key|Value))\b')
Assert-True ($setContext -ge 0 -and $classify -gt $setContext) 'Install-state classification must run immediately after registry context is selected.'
Assert-True (-not $firstRegistryWrite.Success -or $classify -lt $firstRegistryWrite.Index) 'Preflight classification must precede every .onInit registry mutation.'

$functionExpansion = '!insertmacro PCT_HOOK_FUNCTIONS'
Assert-True ([regex]::Matches($script, [regex]::Escape($functionExpansion)).Count -eq 1) 'PCT hook functions must be expanded exactly once after metadata definitions.'
Assert-True ($script.IndexOf('Var OldMainBinaryName', [System.StringComparison]::Ordinal) -lt $script.IndexOf($functionExpansion, [System.StringComparison]::Ordinal)) 'PCT hook functions must expand after product metadata and installer variable definitions.'
Assert-True ($hooks.Contains('!macro PCT_HOOK_FUNCTIONS') -and $hooks.Contains('!macroend')) 'Hook function macro declaration is missing.'

$classification = Get-FunctionBody $hooks 'PCTClassifyInstallState'
foreach ($required in @(
  'Call PCTValidateLegacyRegistration',
  'Call PCTValidatePoverelloRegistration',
  'PCTLegacyOrphanExecutable',
  'PCTPendingMarker',
  'Call PCTRejectInstallState',
  'Call PCTSeedLegacyRegistration'
)) {
  Assert-True ($classification.Contains($required)) "Install-state classification is missing required branch evidence: $required"
}
$legacyValidateAt = $classification.IndexOf('Call PCTValidateLegacyRegistration', [System.StringComparison]::Ordinal)
$newValidateAt = $classification.IndexOf('Call PCTValidatePoverelloRegistration', [System.StringComparison]::Ordinal)
$seedAt = $classification.IndexOf('Call PCTSeedLegacyRegistration', [System.StringComparison]::Ordinal)
Assert-True ($legacyValidateAt -ge 0 -and $newValidateAt -gt $legacyValidateAt -and $seedAt -gt $newValidateAt) 'Registrations must be read/validated before migration values are seeded.'
$existingPoverelloAt = $classification.IndexOf('${If} $PCTNewValid == 1', [System.StringComparison]::Ordinal)
$orphanRejectAt = $classification.IndexOf('${If} $PCTLegacyOrphanExecutable == 1', [System.StringComparison]::Ordinal)
Assert-True ($existingPoverelloAt -ge 0 -and $orphanRejectAt -gt $existingPoverelloAt) 'A valid Poverello registration must be classified before an orphaned legacy executable is rejected.'

$legacyValidation = Get-FunctionBody $hooks 'PCTValidateLegacyRegistration'
foreach ($required in @(
  '"DisplayName"', '"Publisher"', '"DisplayVersion"', '"InstallLocation"',
  '"UninstallString"', '"DisplayIcon"', '"MainBinaryName"',
  '"Software\Sohei IDE (soheidon)\Anthro Bridge"',
  'Call PCTNormalizeAbsolutePath', 'Call PCTNormalizeQuotedExecutable',
  '${MAINBINARYNAME}.exe', 'uninstall.exe'
)) {
  Assert-True ($legacyValidation.Contains($required)) "Legacy registration validation is missing: $required"
}

$installSectionMatch = [regex]::Match($script, '(?s)Section Install\b(.*?)SectionEnd')
Assert-True $installSectionMatch.Success 'Tauri Install section could not be located.'
$installSection = $installSectionMatch.Groups[1].Value
$postInstall = $installSection.IndexOf('!insertmacro NSIS_HOOK_POSTINSTALL', [System.StringComparison]::Ordinal)
Assert-True ($postInstall -ge 0) 'Postinstall verification hook is not called from the production Install section.'
foreach ($required in @('"DisplayName"', '"DisplayIcon"', '"DisplayVersion"', '"Publisher"', '"InstallLocation"', '"UninstallString"', 'WriteUninstaller')) {
  $at = $installSection.IndexOf($required, [System.StringComparison]::Ordinal)
  Assert-True ($at -ge 0 -and $at -lt $postInstall) "Install section does not establish '$required' before postinstall verification."
}
foreach ($shortcut in @('CreateOrUpdateStartMenuShortcut', 'CreateOrUpdateDesktopShortcut')) {
  $guardPattern = '(?s)\$\{If\} \$PCTRegistrationVerified == 1.*?Call ' + [regex]::Escape($shortcut) + '\s*\$\{EndIf\}'
  Assert-True ([regex]::IsMatch($installSection, $guardPattern)) "Install-section call to $shortcut must be inert until registration verification succeeds."
}

$postInstallMacro = [regex]::Match($hooks, '(?s)!macro NSIS_HOOK_POSTINSTALL(.*?)!macroend')
Assert-True $postInstallMacro.Success 'Postinstall hook macro is missing.'
$postInstallBody = $postInstallMacro.Groups[1].Value
$verifyAt = $postInstallBody.IndexOf('Call PCTValidatePoverelloRegistration', [System.StringComparison]::Ordinal)
$verifiedAt = $postInstallBody.IndexOf('StrCpy $PCTRegistrationVerified 1', [System.StringComparison]::Ordinal)
$postVerifyShortcutAt = $postInstallBody.IndexOf('Call CreateOrUpdateStartMenuShortcut', [System.StringComparison]::Ordinal)
$shortcutAt = $postInstallBody.IndexOf('Delete "$SMPROGRAMS\Anthro Bridge.lnk"', [System.StringComparison]::Ordinal)
$migrationCleanupAt = $postInstallBody.IndexOf('Call PCTCompleteLegacyMigration', [System.StringComparison]::Ordinal)
Assert-True ($verifyAt -ge 0 -and $verifiedAt -gt $verifyAt -and $postVerifyShortcutAt -gt $verifiedAt -and $migrationCleanupAt -gt $postVerifyShortcutAt -and $shortcutAt -gt $migrationCleanupAt) 'Legacy cleanup must follow verified Poverello registration and must complete before legacy shortcut retirement.'
Assert-True ($postInstallBody.Contains('$PCTLegacyCleanupFailed == 1') -and $postInstallBody.Contains('SetErrorLevel 1') -and $postInstallBody.Contains('Quit')) 'Legacy cleanup failure must propagate as a nonzero installer failure before shortcut/key retirement.'
Assert-True (-not $postInstallBody.Contains('DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"') -and -not $postInstallBody.Contains('DeleteRegKey HKCU "Software\Sohei IDE (soheidon)\Anthro Bridge"')) 'Legacy registry deletion must use the checked shared cleanup routine.'
Assert-True ($postInstallBody.Contains('SetErrorLevel 1') -and $postInstallBody.Contains('Quit')) 'Failed registration verification must return a nonzero installer result and terminate.'
Assert-True ($postInstallBody.IndexOf('SetErrorLevel 1', [System.StringComparison]::Ordinal) -lt $postInstallBody.IndexOf('Quit', [System.StringComparison]::Ordinal) -and $postInstallBody.IndexOf('Quit', [System.StringComparison]::Ordinal) -lt $verifiedAt) 'A failed postinstall verification must set the nonzero status and exit before registration can be marked verified.'
Assert-True ($postInstallBody -match '(?i)files may be partially installed' -and $postInstallBody -match '(?i)(rerun this installer to retry|rerunning this installer is safe)') 'Failure path must explain partial files and the safe retry route.'
Assert-True ($postInstallBody.Contains('${If} ${Silent}') -and $postInstallBody.Contains('DetailPrint')) 'Silent failure must avoid modal UI while reporting the failure.'

$migrationCleanup = Get-FunctionBody $hooks 'PCTCompleteLegacyMigration'
foreach ($required in @('Call PCTLegacyCleanupWritePending', 'Call PCTLegacyCleanupDeleteKey', 'Call PCTLegacyCleanupProbeKey', 'Call PCTLegacyCleanupVerifyPending', 'Call PCTLegacyCleanupWriteComplete', 'Call PCTLegacyCleanupVerifyComplete', '$PCTLegacyCleanupOperationFailed', '$PCTLegacyCleanupReadbackFailed')) {
  Assert-True ($migrationCleanup.Contains($required)) "Legacy migration cleanup is missing required operation/read-back contract: $required"
}
Assert-True ($migrationCleanup.IndexOf('Call PCTLegacyCleanupWritePending', [System.StringComparison]::Ordinal) -lt $migrationCleanup.IndexOf('Call PCTLegacyCleanupDeleteKey', [System.StringComparison]::Ordinal) -and $migrationCleanup.IndexOf('Call PCTLegacyCleanupDeleteKey', [System.StringComparison]::Ordinal) -lt $migrationCleanup.IndexOf('Call PCTLegacyCleanupWriteComplete', [System.StringComparison]::Ordinal)) 'Durable cleanup-pending state must precede old-key deletion, and completion must follow deletion/read-back.'
$classificationRetry = Get-FunctionBody $hooks 'PCTClassifyInstallState'
Assert-True ($classificationRetry.Contains('PCTLegacyCleanupMarker == 1') -and $classificationRetry.Contains('Call PCTValidateLegacyCleanupResidue')) 'The classifier must recognize only validated partial legacy-cleanup residue as a retryable migration.'
$residueValidator = Get-FunctionBody $hooks 'PCTValidateLegacyCleanupResidue'
Assert-True ($residueValidator.Contains('$PCTPendingPath') -and $residueValidator.Contains('PCTNormalizeQuotedExecutable') -and $residueValidator.Contains('PCTLegacyCleanupProbeKey')) 'Partial-cleanup retry validation must bind every surviving legacy key to the pending install path.'

$preUninstallMacro = [regex]::Match($hooks, '(?s)!macro NSIS_HOOK_PREUNINSTALL(.*?)!macroend')
$uninstallMacro = [regex]::Match($hooks, '(?s)!macro NSIS_HOOK_POSTUNINSTALL(.*?)!macroend')
Assert-True ($preUninstallMacro.Success -and $uninstallMacro.Success) 'Pre/post-uninstall hooks must safely adapt Tauri cleanup to the runtime Poverello data path.'
Assert-True ($preUninstallMacro.Groups[1].Value.Contains('PCTUninstallDeleteRuntimeData $DeleteAppDataCheckboxState') -and $preUninstallMacro.Groups[1].Value.Contains('StrCpy $DeleteAppDataCheckboxState 0')) 'Pre-uninstall must preserve checkbox intent while suppressing Tauri bundle-ID cleanup.'
Assert-True ($uninstallMacro.Groups[1].Value.Contains('PCTUninstallDeleteRuntimeData == 1') -and $uninstallMacro.Groups[1].Value.Contains('un.PCTDeleteStableUserData')) 'The actual delete-app-data checkbox intent must gate the scoped Poverello cleanup hook.'
$uninstallCleanup = Get-FunctionBody $hooks 'un.PCTDeleteStableUserData'
Assert-True ($uninstallCleanup.Contains('$APPDATA\Poverello') -and $uninstallCleanup.Contains('Call un.PCTScanForReparsePoint') -and $uninstallCleanup.Contains('Call un.PCTRemoveDataNode') -and -not $uninstallCleanup.Contains('RmDir /r')) 'Uninstaller cleanup must target only stable runtime data, preflight reparse points, and remove nodes without recursive traversal.'
Assert-True (-not $uninstallCleanup.Contains('Quit') -and $uninstallCleanup.Contains('data-cleanup-failed=1')) 'Data removal must return to the post-hook so registration cleanup runs before failure is reported.'
$registrationRemoval = Get-FunctionBody $hooks 'un.PCTRemovePoverelloRegistration'
Assert-True ($registrationRemoval.Contains('PCT_UN_DELETE_REG_KEY "${MANUPRODUCTKEY}"') -and $registrationRemoval.Contains('PCT_UN_DELETE_REG_KEY "${UNINSTKEY}"') -and $registrationRemoval.Contains('Call un.PCTProbePoverelloKey') -and $registrationRemoval.Contains('$PCTUninstallRegistrationOperationFailed')) 'Uninstall registration cleanup must attempt both product keys, retain operation errors, and verify absence.'
$updateSnapshot = Get-FunctionBody $hooks 'un.PCTSnapshotUpdateRegistration'
$updateRestore = Get-FunctionBody $hooks 'un.PCTRestoreUpdateRegistration'
$updateVerify = Get-FunctionBody $hooks 'un.PCTVerifyUpdateRegistration'
Assert-True ($updateSnapshot.Contains('PCT_UN_READ_REG_STR') -and $updateSnapshot.Contains('un.PCTNormalizeInstallDirectory') -and $updateSnapshot.Contains('PCTUninstallSnapshotValid 1')) 'Update mode must snapshot a complete, path-consistent existing registration before generated key deletion.'
Assert-True ($updateRestore.Contains('PCT_UN_WRITE_REG_STR') -and $updateRestore.Contains('Call un.PCTVerifyUpdateRegistration') -and $updateRestore.Contains('$PCTUninstallRegistrationOperationFailed == 0')) 'Update registration restoration requires successful writes and readback.'
Assert-True ($updateVerify.Contains('PCT_UN_READ_REG_STR') -and $updateVerify.Contains('$PCTSavedPublisherPath') -and $updateVerify.Contains('$PCTSavedUninstallString')) 'Update restoration readback must compare saved registration values.'
$uninstallPostBody = $uninstallMacro.Groups[1].Value
$normalUninstallBranch = $uninstallPostBody.Substring($uninstallPostBody.IndexOf('${Else}', [System.StringComparison]::Ordinal))
Assert-True ($normalUninstallBranch.IndexOf('Call un.PCTDeleteStableUserData', [System.StringComparison]::Ordinal) -lt $normalUninstallBranch.IndexOf('Call un.PCTRemovePoverelloRegistration', [System.StringComparison]::Ordinal) -and $normalUninstallBranch.IndexOf('Call un.PCTRemovePoverelloRegistration', [System.StringComparison]::Ordinal) -lt $normalUninstallBranch.IndexOf('Call un.PCTReportDataCleanupFailure', [System.StringComparison]::Ordinal)) 'Data-delete failure must be reported only after registration cleanup has been attempted.'
$dataFailureReporter = Get-FunctionBody $hooks 'un.PCTReportDataCleanupFailure'
Assert-True ($dataFailureReporter.Contains('$PCTUninstallRegistrationVerified == 1') -and $dataFailureReporter.Contains('registration may remain') -and $dataFailureReporter.Contains('registration was removed')) 'Data cleanup failure reporting must distinguish verified registry cleanup from unresolved registration state.'
$generatedPreUninstallAt = $script.IndexOf('!insertmacro NSIS_HOOK_PREUNINSTALL', [System.StringComparison]::Ordinal)
$generatedDeleteAt = $script.IndexOf('RmDir /r "$APPDATA\${BUNDLEID}"', [System.StringComparison]::Ordinal)
$generatedLocalDeleteAt = $script.IndexOf('RmDir /r "$LOCALAPPDATA\${BUNDLEID}"', [System.StringComparison]::Ordinal)
$generatedPostUninstallAt = $script.IndexOf('!insertmacro NSIS_HOOK_POSTUNINSTALL', [System.StringComparison]::Ordinal)
Assert-True ($generatedPreUninstallAt -ge 0 -and $generatedPreUninstallAt -lt $generatedDeleteAt -and $generatedDeleteAt -ge 0 -and $generatedLocalDeleteAt -gt $generatedDeleteAt -and $generatedPostUninstallAt -gt $generatedLocalDeleteAt) 'The generated uninstaller must suppress Tauri bundle-ID data deletion before its cleanup branch and invoke the scoped runtime hook afterward.'
$generatedUninstallSection = [regex]::Match($script, '(?s)Section Uninstall\b(.*?)SectionEnd')
Assert-True $generatedUninstallSection.Success 'Generated Tauri uninstall section could not be located.'
$generatedUninstallBody = $generatedUninstallSection.Groups[1].Value
$preAt = $generatedUninstallBody.IndexOf('!insertmacro NSIS_HOOK_PREUNINSTALL', [System.StringComparison]::Ordinal)
$uninstKeyAt = $generatedUninstallBody.IndexOf('DeleteRegKey HKCU "${UNINSTKEY}"', [System.StringComparison]::Ordinal)
$checkboxBranchAt = $generatedUninstallBody.IndexOf('${If} $DeleteAppDataCheckboxState = 1', [System.StringComparison]::Ordinal)
$postAt = $generatedUninstallBody.IndexOf('!insertmacro NSIS_HOOK_POSTUNINSTALL', [System.StringComparison]::Ordinal)
Assert-True ($preAt -ge 0 -and $uninstKeyAt -gt $preAt -and $checkboxBranchAt -gt $uninstKeyAt -and $postAt -gt $checkboxBranchAt) 'Actual generated uninstall order must remain PRE hook → unconditional uninstall-key delete → checkbox branch → POST hook.'
Assert-True ((Get-Content -LiteralPath (Join-Path (Split-Path -Parent $HooksPath) '..\..\src\paths.rs') -Raw).Contains('base.join("Poverello")')) 'Runtime stable data path source no longer confirms %APPDATA%\Poverello; review the uninstaller target before proceeding.'

Assert-True ($script.Contains('!define ARCH "x64"') -and $script.Contains('!define DISPLAYLANGUAGESELECTOR "true"')) 'The generated installer must remain x64 and multilingual.'
foreach ($language in @('English', 'Japanese', 'SimpChinese', 'TradChinese', 'Korean', 'French', 'German', 'Spanish')) {
  Assert-True ($script.Contains("$language.nsh")) "Generated installer omits language resource: $language"
}
Assert-True ($script.Contains('MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"') -and $script.Contains('MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"')) 'Installer/uninstaller header image macros are missing.'
Assert-True ($script.Contains('icons\header_image.bmp')) 'Generated NSIS metadata does not reference the Poverello header image.'
Assert-True ($script -match '(?m)^!define INSTALLERICON ".*\\icons\\icon\.ico"\r?$' -and $script -match '(?m)^!define UNINSTALLERICON ".*\\icons\\icon\.ico"\r?$') 'Installer/uninstaller executable icons are not set to the Poverello ICO.'

$buildHelper = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot 'build-windows-nsis.ps1'))
Assert-True ($buildHelper.Contains('Poverello-Coding-Tools_${appVersion}_x64-setup.exe')) 'Build helper does not use the canonical no-space artifact basename.'
Assert-True ($buildHelper.Contains("CARGO_PROFILE_RELEASE_CODEGEN_UNITS', '1'")) 'Build helper must set the known-good release codegen setting explicitly.'
Assert-True ($buildHelper.Contains('Start-Transcript') -and $buildHelper.Contains('Source HEAD:') -and $buildHelper.Contains('Final installer SHA-256:')) 'Build helper must persist source/build/artifact provenance.'
Assert-True ($buildHelper.Contains("Groups[1].Value -ne 'currentUser'")) 'Build helper must reject install modes whose registry writes are not covered by the current recovery snapshot.'
Assert-True ($buildHelper.Contains('resolving uninstall function "un.PCTDeleteStableUserData"') -and $buildHelper.Contains('resolving install function "PCTValidatePoverelloRegistration"')) 'Build helper must tolerate only the two known first-pass unresolved calls to functions expanded from the current PCT hook macro.'

$recovery = Get-FunctionBody $hooks 'PCTRestoreRegistrationOnFailure'
$savedVerifier = Get-FunctionBody $hooks 'PCTVerifySavedPoverelloRegistration'
$removedVerifier = Get-FunctionBody $hooks 'PCTVerifyNewRegistrationRemoved'
$legacyVerifier = Get-FunctionBody $hooks 'PCTVerifyLegacyRegistrationUnchanged'
Assert-True ($recovery.IndexOf('Call PCTVerifySavedPoverelloRegistration', [System.StringComparison]::Ordinal) -gt $recovery.IndexOf('Call PCTRecoveryWriteString', [System.StringComparison]::Ordinal)) 'Existing registration recovery must read back after writes.'
foreach ($operation in @('PCTRecoveryWriteString', 'PCTRecoveryWriteDword', 'PCTRecoveryDeleteValue', 'PCTRecoveryDeleteKey', 'PCTRecoveryDeleteKeyIfEmpty')) {
  $operationBody = Get-FunctionBody $hooks $operation
  Assert-True ($operationBody.Contains('$PCTRecoveryOperationFailed 1')) "Recovery operation '$operation' must preserve an operation-level failure before read-back."
}
Assert-True (-not $savedVerifier.Contains('StrCpy $PCTRecoveryFailed 0')) 'Saved-registration read-back must not clear an earlier recovery failure.'
Assert-True ($recovery.Contains('${AndIf} $PCTRecoveryOperationFailed == 0')) 'Recovery cannot be marked verified unless every restoration operation succeeded.'
Assert-True ($recovery.IndexOf('Call PCTVerifyNewRegistrationRemoved', [System.StringComparison]::Ordinal) -ge 0 -and $recovery.IndexOf('Call PCTVerifyLegacyRegistrationUnchanged', [System.StringComparison]::Ordinal) -gt $recovery.IndexOf('Call PCTVerifyNewRegistrationRemoved', [System.StringComparison]::Ordinal)) 'Fresh/migration recovery must verify removal and preserve legacy registration.'
Assert-True ($recovery.IndexOf('StrCpy $PCTCleanupDone 1', [System.StringComparison]::Ordinal) -gt $recovery.IndexOf('${If} $PCTRecoveryFailed == 0', [System.StringComparison]::Ordinal)) 'Cleanup may be marked complete only after successful read-back.'
foreach ($required in @('PCTSavedNoModifyExists', 'PCTSavedNoRepairExists', 'PCTSavedEstimatedSizeExists')) {
  Assert-True ($savedVerifier.Contains($required)) "Recovery verifier omits saved optional-value presence: $required"
}
Assert-True ($removedVerifier.Contains('Call PCTDeleteRegistrationKeyIfPresent') -and $removedVerifier.Contains('Call PCTVerifyRegistryKeyAbsent')) 'New-registration cleanup must confirm keys are absent after deletion.'
Assert-True ($legacyVerifier.Contains('$PCTLegacyRawPath')) 'Legacy recovery must compare the original publisher path representation.'
Assert-True ($legacyVerifier.Contains('PCTLegacyDisplayIconExists')) 'Legacy recovery must preserve optional DisplayIcon presence/value.'
Assert-True ($script -match '(?m)^!define HOMEPAGE ""\r?$') 'Unexpected non-empty homepage values require snapshot/restore coverage before installation.'

# Execute the production path-normalization functions in a disposable NSIS
# harness. The harness calls only PCTNormalizeAbsolutePath and
# PCTNormalizeQuotedExecutable; it never invokes the installer or registry APIs.
$nsisCompiler = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
Assert-True (Test-Path -LiteralPath $nsisCompiler -PathType Leaf) 'NSIS compiler is required for production parser behavior tests.'
$harnessRoot = Join-Path (Split-Path -Parent (Split-Path -Parent $InstallerScript)) ("build-logs\nsis-path-tests-{0}" -f [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $harnessRoot -Force | Out-Null
$testDirectory = Join-Path $harnessRoot 'Path With Spaces'
New-Item -ItemType Directory -Path $testDirectory -Force | Out-Null
$testExecutable = Join-Path $testDirectory 'uninstall.exe'
[System.IO.File]::WriteAllBytes($testExecutable, [byte[]]@(0x4D, 0x5A))
$expectedDirectory = [System.IO.Path]::GetFullPath($testDirectory).TrimEnd('\').ToLowerInvariant()
$expectedExecutable = (Join-Path $expectedDirectory 'uninstall.exe').ToLowerInvariant()
$hooksAbsolute = (Resolve-Path -LiteralPath $HooksPath).Path
$resultPath = Join-Path $harnessRoot 'path-results.txt'
$harnessSource = @"
Unicode true
RequestExecutionLevel user
!include LogicLib.nsh
!include FileFunc.nsh
!include StrFunc.nsh
`${StrCase}
!define PRODUCTNAME "Poverello Coding Tools"
!define MANUFACTURER "Sohei IDE (soheidon)"
!define VERSION "0.24.1"
!define MAINBINARYNAME "anthro-bridge"
!define MANUKEY "Software\`${MANUFACTURER}"
!define MANUPRODUCTKEY "`${MANUKEY}\`${PRODUCTNAME}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\`${PRODUCTNAME}"
Var UpdateMode
!include "$hooksAbsolute"
!insertmacro PCT_HOOK_FUNCTIONS
OutFile "$harnessRoot\path-harness.exe"
SilentInstall silent
Section
  StrCpy `$PCTUninstallDeleteRuntimeData 0
  FileOpen `$0 "$resultPath" w
  StrCpy `$PCTNormalizeInput "`$\"$testDirectory`$\""
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "quoted=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "$testDirectory\"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "unquoted=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput ""
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "empty=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "   "
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "whitespace=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput ".\Poverello"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "relative=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "C:Poverello"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "drive-relative=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "\\server\share\Poverello"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "unc=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "\\?\C:\Poverello"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "device=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "`$\"C:\Poverello"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "unmatched=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "$testDirectory`$\"evil"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "embedded=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "`$\"$testDirectory`$\" /S"
  Call PCTNormalizeAbsolutePath
  FileWrite `$0 "suffix=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "`$\"$expectedExecutable`$\""
  Call PCTNormalizeQuotedExecutable
  FileWrite `$0 "quoted-executable=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "$expectedExecutable"
  Call PCTNormalizeQuotedExecutable
  FileWrite `$0 "unquoted-executable=`$PCTNormalizeOutput`$\n"
  StrCpy `$PCTNormalizeInput "`$\"$expectedExecutable`$\" /S"
  Call PCTNormalizeQuotedExecutable
  FileWrite `$0 "executable-arguments=`$PCTNormalizeOutput`$\n"
  ; Read-only probe of the same HKEY_CURRENT_USER pseudo-handle used by the
  ; recovery verifier. Open only the standard Software key and close it.
  System::Call 'advapi32::RegOpenKeyExW(p 0x80000001, w "Software", i 0, i 0x20019, *p .r2)i.r3'
  FileWrite `$0 "hkcu-software-open-status=`$3`$\n"
  `${If} `$3 == 0
    System::Call 'advapi32::RegCloseKey(p r2)'
  `${EndIf}
  FileClose `$0
SectionEnd
"@
$harnessScript = Join-Path $harnessRoot 'path-harness.nsi'
[System.IO.File]::WriteAllText($harnessScript, $harnessSource, [System.Text.UTF8Encoding]::new($false))
$compileOutput = & $nsisCompiler /V2 $harnessScript 2>&1
$compileExit = $LASTEXITCODE
$compileOutput | ForEach-Object { Write-Output $_ }
Assert-True ($compileExit -eq 0) "Production path harness compilation failed with exit code $compileExit."
$harnessExe = Join-Path $harnessRoot 'path-harness.exe'
$run = Start-Process -FilePath $harnessExe -WindowStyle Hidden -Wait -PassThru
Assert-True ($run.ExitCode -eq 0) "Production path harness exited with $($run.ExitCode)."
Assert-True (Test-Path -LiteralPath $resultPath -PathType Leaf) 'Production path harness did not produce its result file.'
$pathResults = @{}
foreach ($line in Get-Content -LiteralPath $resultPath) {
  $parts = $line -split '=', 2
  $pathResults[$parts[0]] = $parts[1]
}
$canonical = $expectedDirectory
Assert-True ($pathResults['quoted'] -eq $canonical) "Quoted directory did not normalize as expected: '$($pathResults['quoted'])'."
Assert-True ($pathResults['unquoted'] -eq $canonical) "Unquoted/trailing-separator directory did not normalize as expected: '$($pathResults['unquoted'])'."
foreach ($negative in @('empty', 'whitespace', 'relative', 'drive-relative', 'unc', 'device', 'unmatched', 'embedded', 'suffix', 'unquoted-executable', 'executable-arguments')) {
  Assert-True ([string]::IsNullOrEmpty($pathResults[$negative])) "Malformed path case '$negative' was accepted as '$($pathResults[$negative])'."
}
Assert-True ($pathResults['quoted-executable'] -eq $expectedExecutable) 'Quoted executable path did not normalize as expected.'
Assert-True ($pathResults['hkcu-software-open-status'] -eq '0') 'The production registry-verification HKEY_CURRENT_USER handle did not open the read-only Software key.'
Write-Output "Production parser behavior harness: PASS ($harnessExe)"

# Execute the shared production recovery function with its compile-time-only
# in-memory registry backend. This harness never calls RegOpen/Read/Write or
# touches the user's HKCU product keys.
$recoveryResultPath = Join-Path $harnessRoot 'recovery-results.txt'
$recoverySource = @"
Unicode true
RequestExecutionLevel user
!define PCT_NSIS_TEST_BACKEND
!include LogicLib.nsh
!include FileFunc.nsh
!include StrFunc.nsh
`${StrCase}
!define PRODUCTNAME "Poverello Coding Tools"
!define MANUFACTURER "Sohei IDE (soheidon)"
!define VERSION "0.24.1"
!define MAINBINARYNAME "anthro-bridge"
!define MANUKEY "Software\`${MANUFACTURER}"
!define MANUPRODUCTKEY "`${MANUKEY}\`${PRODUCTNAME}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\`${PRODUCTNAME}"
Var UpdateMode
!include "$hooksAbsolute"
!insertmacro PCT_HOOK_FUNCTIONS
OutFile "$harnessRoot\recovery-harness.exe"
SilentInstall silent
Function ResetFake
  StrCpy `$PCTTestFailOperation ""
  StrCpy `$PCTTestFailKey ""
  StrCpy `$PCTTestFailValue ""
  StrCpy `$PCTFakePublisherKeyExists 0
  StrCpy `$PCTFakePublisherPath ""
  StrCpy `$PCTFakeUninstallKeyExists 0
  StrCpy `$PCTFakeDisplayName ""
  StrCpy `$PCTFakeDisplayIcon ""
  StrCpy `$PCTFakeDisplayVersion ""
  StrCpy `$PCTFakePublisher ""
  StrCpy `$PCTFakeInstallLocation ""
  StrCpy `$PCTFakeUninstallString ""
  StrCpy `$PCTFakeMainBinaryName ""
  StrCpy `$PCTFakeNoModifyExists 0
  StrCpy `$PCTFakeNoModify 0
  StrCpy `$PCTFakeNoRepairExists 0
  StrCpy `$PCTFakeNoRepair 0
  StrCpy `$PCTFakeEstimatedSizeExists 0
  StrCpy `$PCTFakeEstimatedSize 0
  StrCpy `$PCTFakeLegacyUninstallKeyExists 0
  StrCpy `$PCTFakeLegacyPublisherKeyExists 0
  StrCpy `$PCTRecoveryVerified 0
  StrCpy `$PCTRecoveryFailed 0
  StrCpy `$PCTRecoveryOperationFailed 0
  StrCpy `$PCTRecoveryAttempted 0
  StrCpy `$PCTCleanupDone 0
  StrCpy `$PCTRegistrationVerified 0
  StrCpy `$PCTHadExistingPoverello 1
  StrCpy `$PCTLegacyUpgradePending 0
  StrCpy `$PCTTestMissingMainBinary ""
FunctionEnd
Function SeedExistingSnapshot
  Call ResetFake
  StrCpy `$PCTFakePublisherKeyExists 1
  StrCpy `$PCTFakePublisherPath "C:\Poverello"
  StrCpy `$PCTFakeUninstallKeyExists 1
  StrCpy `$PCTFakeDisplayName "Poverello Coding Tools"
  StrCpy `$PCTFakeDisplayIcon "C:\Poverello\anthro-bridge.exe"
  StrCpy `$PCTFakeDisplayVersion "0.24.1"
  StrCpy `$PCTFakePublisher "Sohei IDE (soheidon)"
  StrCpy `$PCTFakeInstallLocation "`$\"C:\Poverello`$\""
  StrCpy `$PCTFakeUninstallString "`$\"C:\Poverello\uninstall.exe`$\""
  StrCpy `$PCTFakeMainBinaryName "anthro-bridge.exe"
  StrCpy `$PCTFakeNoModifyExists 1
  StrCpy `$PCTFakeNoModify 99
  StrCpy `$PCTFakeNoRepairExists 1
  StrCpy `$PCTFakeNoRepair 77
  StrCpy `$PCTFakeEstimatedSizeExists 1
  StrCpy `$PCTFakeEstimatedSize 9
  StrCpy `$PCTSavedPublisherPath "C:\Poverello"
  StrCpy `$PCTSavedDisplayName "Poverello Coding Tools"
  StrCpy `$PCTSavedDisplayIcon "C:\Poverello\anthro-bridge.exe"
  StrCpy `$PCTSavedVersion "0.24.1"
  StrCpy `$PCTSavedPublisher "Sohei IDE (soheidon)"
  StrCpy `$PCTSavedInstallLocation "`$\"C:\Poverello`$\""
  StrCpy `$PCTSavedUninstallString "`$\"C:\Poverello\uninstall.exe`$\""
  StrCpy `$PCTSavedMainBinaryName "anthro-bridge.exe"
  StrCpy `$PCTSavedNoModify "1"
  StrCpy `$PCTSavedNoModifyExists 1
  StrCpy `$PCTSavedNoRepair "0"
  StrCpy `$PCTSavedNoRepairExists 0
  StrCpy `$PCTSavedEstimatedSize "1234"
  StrCpy `$PCTSavedEstimatedSizeExists 1
FunctionEnd
Function SeedLegacyMigration
  Call ResetFake
  StrCpy `$PCTHadExistingPoverello 0
  StrCpy `$PCTLegacyUpgradePending 1
  StrCpy `$PCTFakePublisherKeyExists 1
  StrCpy `$PCTFakePublisherPath "C:\Poverello"
  StrCpy `$PCTFakeUninstallKeyExists 1
  StrCpy `$PCTFakeLegacyUninstallKeyExists 1
  StrCpy `$PCTFakeLegacyPublisherKeyExists 1
  StrCpy `$PCTFakeDisplayName "incomplete new registration"
  StrCpy `$PCTFakeLegacyDisplayName "Anthro Bridge"
  StrCpy `$PCTFakeLegacyPublisher "Sohei IDE (soheidon)"
  StrCpy `$PCTFakeLegacyVersion "0.24.0"
  StrCpy `$PCTFakeLegacyInstallLocation "`$\"C:\Poverello`$\""
  StrCpy `$PCTFakeLegacyUninstallString "`$\"C:\Poverello\uninstall.exe`$\""
  StrCpy `$PCTFakeLegacyDisplayIcon "C:\Poverello\anthro-bridge.exe"
  StrCpy `$PCTLegacyDisplayIconExists 1
  StrCpy `$PCTFakeLegacyMainBinaryName "anthro-bridge.exe"
  StrCpy `$PCTFakeLegacyPublisherPath "`$\"C:\Poverello`$\""
  StrCpy `$PCTLegacyDisplayName "Anthro Bridge"
  StrCpy `$PCTLegacyPublisher "Sohei IDE (soheidon)"
  StrCpy `$PCTLegacyVersion "0.24.0"
  StrCpy `$PCTLegacyInstallLocation "`$\"C:\Poverello`$\""
  StrCpy `$PCTLegacyUninstallString "`$\"C:\Poverello\uninstall.exe`$\""
  StrCpy `$PCTLegacyDisplayIcon "C:\Poverello\anthro-bridge.exe"
  StrCpy `$PCTLegacyMainBinaryName "anthro-bridge.exe"
  StrCpy `$PCTLegacyRawPath "`$\"C:\Poverello`$\""
FunctionEnd
Function SeedCleanInstall
  Call ResetFake
  StrCpy `$PCTHadExistingPoverello 0
  StrCpy `$PCTFakePublisherKeyExists 1
  StrCpy `$PCTFakePublisherPath "C:\Poverello"
  StrCpy `$PCTFakeUninstallKeyExists 1
  StrCpy `$PCTFakeDisplayName "incomplete clean-install registration"
FunctionEnd
Function WriteRecoveryState
  FileWrite `$R9 "`$R8|verified=`$PCTRecoveryVerified|cleanup=`$PCTCleanupDone|failed=`$PCTRecoveryFailed|operationFailed=`$PCTRecoveryOperationFailed|registrationVerified=`$PCTRegistrationVerified|newUninstall=`$PCTFakeUninstallKeyExists|newPublisher=`$PCTFakePublisherKeyExists|legacy=`$PCTFakeLegacyUninstallKeyExists|noModify=`$PCTFakeNoModifyExists:`$PCTFakeNoModify|noRepair=`$PCTFakeNoRepairExists:`$PCTFakeNoRepair|size=`$PCTFakeEstimatedSizeExists:`$PCTFakeEstimatedSize`$\n"
FunctionEnd
Section
  FileOpen `$R9 "$recoveryResultPath" w
  StrCpy `$R8 "restore-success"
  Call SeedExistingSnapshot
  StrCpy `$PCTFakeDisplayVersion "changed"
  StrCpy `$PCTFakeNoModify 99
  StrCpy `$PCTFakeNoRepairExists 1
  StrCpy `$PCTFakeNoRepair 77
  StrCpy `$PCTFakeEstimatedSize 9
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "write-failure"
  Call SeedExistingSnapshot
  StrCpy `$PCTFakeDisplayVersion "changed"
  StrCpy `$PCTTestFailOperation "write"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  StrCpy `$PCTTestFailValue "DisplayVersion"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "write-failure-already-matching"
  Call SeedExistingSnapshot
  ; DisplayName already equals the saved value; the injected failed write must
  ; still fail recovery even though final read-back would otherwise match.
  StrCpy `$PCTTestFailOperation "write"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  StrCpy `$PCTTestFailValue "DisplayName"
  StrCpy `$PCTFakeLegacyUninstallKeyExists 1
  StrCpy `$PCTFakeLegacyPublisherKeyExists 1
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "readback-failure"
  Call SeedExistingSnapshot
  StrCpy `$PCTTestFailOperation "readback"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  StrCpy `$PCTTestFailValue "DisplayName"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "legacy-removal-success"
  Call SeedLegacyMigration
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "clean-install-removal-success"
  Call SeedCleanInstall
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "delete-failure"
  Call SeedLegacyMigration
  StrCpy `$PCTTestFailOperation "delete-key"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "access-failure"
  Call SeedLegacyMigration
  StrCpy `$PCTTestFailOperation "access"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "optional-delete-failure"
  Call SeedExistingSnapshot
  StrCpy `$PCTFakeNoRepairExists 1
  StrCpy `$PCTFakeNoRepair 77
  StrCpy `$PCTTestFailOperation "delete-value"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  StrCpy `$PCTTestFailValue "NoRepair"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "delete-failure-already-absent"
  Call SeedExistingSnapshot
  ; This saved snapshot says NoRepair was absent; make the live fake value
  ; already absent too, so only the failed delete operation can make recovery fail.
  StrCpy `$PCTFakeNoRepairExists 0
  StrCpy `$PCTFakeNoRepair 0
  StrCpy `$PCTTestFailOperation "delete-value"
  StrCpy `$PCTTestFailKey "`${UNINSTKEY}"
  StrCpy `$PCTTestFailValue "NoRepair"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState

  StrCpy `$R8 "missing-binary-cleanup"
  Call SeedExistingSnapshot
  StrCpy `$PCTTestMissingMainBinary "1"
  Call PCTRestoreRegistrationOnFailure
  Call WriteRecoveryState
  FileClose `$R9
SectionEnd
"@
$recoveryScript = Join-Path $harnessRoot 'recovery-harness.nsi'
[System.IO.File]::WriteAllText($recoveryScript, $recoverySource, [System.Text.UTF8Encoding]::new($false))
$previousErrorActionPreference = $ErrorActionPreference
try {
  $ErrorActionPreference = 'Continue'
  $recoveryCompileOutput = & $nsisCompiler /V2 $recoveryScript 2>&1
  $recoveryCompileExit = $LASTEXITCODE
}
finally {
  $ErrorActionPreference = $previousErrorActionPreference
}
$recoveryCompileOutput | ForEach-Object { Write-Output $_ }
Assert-True ($recoveryCompileExit -eq 0) "Recovery harness compilation failed with exit code $recoveryCompileExit."
$recoveryExe = Join-Path $harnessRoot 'recovery-harness.exe'
$recoveryRun = Start-Process -FilePath $recoveryExe -WindowStyle Hidden -Wait -PassThru
Assert-True ($recoveryRun.ExitCode -eq 0) "Recovery harness exited with $($recoveryRun.ExitCode)."
Assert-True (Test-Path -LiteralPath $recoveryResultPath -PathType Leaf) 'Recovery harness did not produce its result file.'
$recoveryResults = @{}
foreach ($line in Get-Content -LiteralPath $recoveryResultPath) {
  $parts = $line -split '\|', 2
  $recoveryResults[$parts[0]] = $parts[1]
}
Assert-True ($recoveryResults['restore-success'] -match 'verified=1\|cleanup=1\|failed=0' -and $recoveryResults['restore-success'] -match 'noModify=1:1\|noRepair=0:0\|size=1:1234') 'Successful restoration did not verify all saved values and optional-value presence.'
foreach ($case in @('write-failure', 'write-failure-already-matching', 'readback-failure', 'delete-failure', 'access-failure', 'optional-delete-failure', 'delete-failure-already-absent')) {
  Assert-True ($recoveryResults[$case] -match 'verified=0\|cleanup=0\|failed=1') "Injected recovery failure '$case' was incorrectly reported as verified."
}
Assert-True ($recoveryResults['write-failure-already-matching'] -match 'operationFailed=1\|registrationVerified=0' -and $recoveryResults['write-failure-already-matching'] -match 'legacy=1') 'A failed restoration write with matching read-back must remain failed, keep registration unverified, and preserve legacy registration.'
Assert-True ($recoveryResults['delete-failure-already-absent'] -match 'operationFailed=1' -and $recoveryResults['delete-failure-already-absent'] -match 'noRepair=0:0') 'A failed delete of an already-absent optional value must remain an operation failure.'
Assert-True ($recoveryResults['legacy-removal-success'] -match 'verified=1\|cleanup=1\|failed=0\|operationFailed=0\|registrationVerified=0\|newUninstall=0\|newPublisher=0\|legacy=1') 'Migration cleanup did not remove only new registrations and preserve the legacy registration.'
Assert-True ($recoveryResults['clean-install-removal-success'] -match 'verified=1\|cleanup=1\|failed=0\|operationFailed=0\|registrationVerified=0\|newUninstall=0\|newPublisher=0\|legacy=0') 'Clean-install recovery did not verify removal of incomplete Poverello registrations.'
Assert-True ($recoveryResults['missing-binary-cleanup'] -match 'verified=1\|cleanup=1\|failed=0\|operationFailed=0\|registrationVerified=0\|newUninstall=0\|newPublisher=0') 'Missing main binary must not restore old registration and must clean up instead.'
Assert-True (-not $recoverySource.Contains('RegOpenKeyExW') -and -not $recoverySource.Contains('ReadRegStr') -and -not $recoverySource.Contains('WriteRegStr') -and -not $recoverySource.Contains('DeleteRegKey')) 'Recovery fake-backend harness must not contain operating-system registry operations.'
Assert-True (-not $script.Contains('PCTTestRegistry') -and -not $script.Contains('PCT_NSIS_TEST_BACKEND') -and -not $hooks.Contains('!define PCT_NSIS_TEST_BACKEND')) 'Production-generated installer unexpectedly includes or enables the test backend.'
Write-Output "Production recovery-path harness: PASS ($recoveryExe)"

# Exercise the production migration-cleanup decision path against the same
# compile-time-only in-memory backend. No product registry keys are touched.
$migrationResultPath = Join-Path $harnessRoot 'migration-results.txt'
$migrationSource = @"
Unicode true
RequestExecutionLevel user
!define PCT_NSIS_TEST_BACKEND
!include LogicLib.nsh
!include FileFunc.nsh
!include StrFunc.nsh
`${StrCase}
!define PRODUCTNAME "Poverello Coding Tools"
!define MANUFACTURER "Sohei IDE (soheidon)"
!define VERSION "0.24.1"
!define MAINBINARYNAME "anthro-bridge"
!define MANUKEY "Software\`${MANUFACTURER}"
!define MANUPRODUCTKEY "`${MANUKEY}\`${PRODUCTNAME}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\`${PRODUCTNAME}"
Var UpdateMode
!include "$hooksAbsolute"
!insertmacro PCT_HOOK_FUNCTIONS
OutFile "$harnessRoot\migration-harness.exe"
SilentInstall silent
Function SeedMigration
  StrCpy `$PCTTestFailOperation ""
  StrCpy `$PCTTestFailKey ""
  StrCpy `$PCTFakeLegacyUninstallKeyExists 1
  StrCpy `$PCTFakeLegacyPublisherKeyExists 1
  StrCpy `$PCTFakeLegacyDisplayName "Anthro Bridge"
  StrCpy `$PCTFakeLegacyPublisher "Sohei IDE (soheidon)"
  StrCpy `$PCTFakeLegacyVersion "0.24.0"
  StrCpy `$PCTFakeLegacyInstallLocation "`$\"$testDirectory`$\""
  StrCpy `$PCTFakeLegacyUninstallString "`$\"$testExecutable`$\""
  StrCpy `$PCTFakeLegacyMainBinaryName "anthro-bridge.exe"
  StrCpy `$PCTFakeLegacyPublisherPath "`$\"$testDirectory`$\""
  StrCpy `$PCTLegacyDisplayName "Anthro Bridge"
  StrCpy `$PCTLegacyPublisher "Sohei IDE (soheidon)"
  StrCpy `$PCTLegacyVersion "0.24.0"
  StrCpy `$PCTLegacyInstallLocation "`$\"$testDirectory`$\""
  StrCpy `$PCTLegacyUninstallString "`$\"$testExecutable`$\""
  StrCpy `$PCTLegacyMainBinaryName "anthro-bridge.exe"
  StrCpy `$PCTLegacyRawPath "`$\"$testDirectory`$\""
  StrCpy `$PCTPendingPath "$expectedDirectory"
  StrCpy `$PCTFakeLegacyCleanupMarker 0
  StrCpy `$PCTFakeLegacyCompleteMarker 0
  StrCpy `$PCTLegacyUpgradePending 1
  StrCpy `$PCTLegacyUpgradeComplete 0
  StrCpy `$PCTLegacyCleanupFailed 0
FunctionEnd
Function WriteMigrationState
  FileWrite `$R9 "`$R8|failed=`$PCTLegacyCleanupFailed|operationFailed=`$PCTLegacyCleanupOperationFailed|readbackFailed=`$PCTLegacyCleanupReadbackFailed|pending=`$PCTFakeLegacyCleanupMarker|completeMarker=`$PCTFakeLegacyCompleteMarker|complete=`$PCTLegacyUpgradeComplete|uninstallKey=`$PCTFakeLegacyUninstallKeyExists|publisherKey=`$PCTFakeLegacyPublisherKeyExists`$\n"
FunctionEnd
Section
  FileOpen `$R9 "$migrationResultPath" w
  StrCpy `$R8 "success"
  Call SeedMigration
  Call PCTCompleteLegacyMigration
  Call WriteMigrationState

  StrCpy `$R8 "uninstall-delete-failure"
  Call SeedMigration
  StrCpy `$PCTTestFailOperation "legacy-delete-key"
  StrCpy `$PCTTestFailKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
  Call PCTCompleteLegacyMigration
  Call WriteMigrationState
  Call PCTValidateLegacyCleanupResidue
  FileWrite `$R9 "retry-residue|failed=`$PCTLegacyCleanupReadbackFailed`$\n"
  StrCpy `$PCTNormalizeInput `$PCTLegacyInstallLocation
  Call PCTNormalizeAbsolutePath
  FileWrite `$R9 "retry-install-path|norm=`$PCTNormalizeOutput|pending=`$PCTPendingPath`$\n"
  StrCpy `$PCTNormalizeInput `$PCTLegacyUninstallString
  Call PCTNormalizeQuotedExecutable
  FileWrite `$R9 "retry-uninstall-path|norm=`$PCTNormalizeOutput|raw=`$PCTLegacyUninstallString`$\n"
  Call PCTCompleteLegacyMigration
  StrCpy `$R8 "retry-after-uninstall-delete-failure"
  Call WriteMigrationState

  StrCpy `$R8 "publisher-delete-failure"
  Call SeedMigration
  StrCpy `$PCTTestFailOperation "legacy-delete-key"
  StrCpy `$PCTTestFailKey "Software\Sohei IDE (soheidon)\Anthro Bridge"
  Call PCTCompleteLegacyMigration
  Call WriteMigrationState
  Call PCTValidateLegacyCleanupResidue
  FileWrite `$R9 "publisher-only-residue|failed=`$PCTLegacyCleanupReadbackFailed`$\n"

  StrCpy `$R8 "readback-access-failure"
  Call SeedMigration
  StrCpy `$PCTTestFailOperation "legacy-probe-readback"
  StrCpy `$PCTTestFailKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
  Call PCTCompleteLegacyMigration
  Call WriteMigrationState

  StrCpy `$R8 "complete-marker-write-failure"
  Call SeedMigration
  StrCpy `$PCTTestFailOperation "legacy-write-complete"
  Call PCTCompleteLegacyMigration
  Call WriteMigrationState
  FileClose `$R9
SectionEnd
"@
$migrationScript = Join-Path $harnessRoot 'migration-harness.nsi'
[System.IO.File]::WriteAllText($migrationScript, $migrationSource, [System.Text.UTF8Encoding]::new($false))
$migrationCompileOutput = & $nsisCompiler /V2 $migrationScript 2>&1
$migrationCompileExit = $LASTEXITCODE
$migrationCompileOutput | ForEach-Object { Write-Output $_ }
Assert-True ($migrationCompileExit -eq 0) "Migration cleanup harness compilation failed with exit code $migrationCompileExit."
$migrationExe = Join-Path $harnessRoot 'migration-harness.exe'
$migrationRun = Start-Process -FilePath $migrationExe -WindowStyle Hidden -Wait -PassThru
Assert-True ($migrationRun.ExitCode -eq 0) "Migration cleanup harness exited with $($migrationRun.ExitCode)."
$migrationResults = @{}
foreach ($line in Get-Content -LiteralPath $migrationResultPath) {
  $parts = $line -split '\|', 2
  $migrationResults[$parts[0]] = $parts[1]
}
Assert-True ($migrationResults['success'] -match 'failed=0\|operationFailed=0\|readbackFailed=0\|pending=1\|completeMarker=1\|complete=1\|uninstallKey=0\|publisherKey=0') 'Successful migration cleanup did not verify deletion before recording completion.'
foreach ($case in @('uninstall-delete-failure', 'publisher-delete-failure', 'readback-access-failure', 'complete-marker-write-failure')) {
  Assert-True ($migrationResults[$case] -match 'failed=1\|.*complete=0') "Migration cleanup case '$case' was incorrectly reported complete."
}
Assert-True ($migrationResults['uninstall-delete-failure'] -match 'operationFailed=1\|.*pending=1\|completeMarker=0\|complete=0\|uninstallKey=1') 'A failed legacy uninstall-key deletion must retain pending recovery state and the surviving registration.'
Assert-True ($migrationResults['retry-residue'] -eq 'failed=0') 'A valid remaining uninstall registration was not accepted as a cleanup retry residue.'
Assert-True ($migrationResults['retry-after-uninstall-delete-failure'] -match 'failed=0\|operationFailed=0\|readbackFailed=0\|pending=1\|completeMarker=1\|complete=1\|uninstallKey=0\|publisherKey=0') 'Retry after partial legacy cleanup did not complete idempotently.'
Assert-True ($migrationResults['publisher-only-residue'] -eq 'failed=0') 'A publisher-key-only cleanup residue bound to the pending path was not accepted for retry.'
Assert-True ($migrationResults['readback-access-failure'] -match 'readbackFailed=1\|.*completeMarker=0\|complete=0') 'Registry read-back access failure did not prevent migration completion.'
Assert-True (-not $migrationSource.Contains('RegOpenKeyExW') -and -not $migrationSource.Contains('DeleteRegKey')) 'Migration fake-backend harness must not touch operating-system registry operations.'
Write-Output "Production migration-cleanup decision harness: PASS ($migrationExe)"

# Prove restart recovery through the real startup classifier. Each orientation
# uses two separate NSIS process invocations and a durable INI-backed fake HKCU
# store. The compile-time test backend routes classifier reads and cleanup
# mutations to this fixture; production builds retain native HKCU operations.
$freshRetrySource = @"
Unicode true
RequestExecutionLevel user
!define PCT_NSIS_TEST_BACKEND
!include LogicLib.nsh
!include FileFunc.nsh
!include StrFunc.nsh
`${StrCase}
!define PRODUCTNAME "Poverello Coding Tools"
!define MANUFACTURER "Sohei IDE (soheidon)"
!define VERSION "0.24.1"
!define MAINBINARYNAME "anthro-bridge"
!define MANUKEY "Software\`${MANUFACTURER}"
!define MANUPRODUCTKEY "`${MANUKEY}\`${PRODUCTNAME}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\`${PRODUCTNAME}"
Var UpdateMode
!include "$hooksAbsolute"
!insertmacro PCT_HOOK_FUNCTIONS
OutFile "$harnessRoot\fresh-retry-harness.exe"
InstallDir "$harnessRoot\not-the-migration-target"
SilentInstall silent
Section
  ReadEnvStr `$PCTTestRegistryStatePath "PCT_NSIS_TEST_REGISTRY_STATE"
  ReadEnvStr `$R8 "PCT_NSIS_TEST_PHASE"
  StrCpy `$PCTTestFailOperation ""
  StrCpy `$PCTTestFailKey ""
  `${If} `$R8 == "stage"
    ReadEnvStr `$PCTTestFailKey "PCT_NSIS_TEST_FAIL_KEY"
    StrCpy `$PCTTestFailOperation "legacy-delete-key"
    Call PCTCompleteLegacyMigration
    FileOpen `$R9 "$harnessRoot\fresh-stage-result.txt" w
    FileWrite `$R9 "failed=`$PCTLegacyCleanupFailed|complete=`$PCTLegacyUpgradeComplete|pending=`$PCTFakeLegacyCleanupMarker`$\n"
    FileClose `$R9
  `${ElseIf} `$R8 == "retry"
    Call PCTClassifyInstallState
    FileOpen `$R9 "$harnessRoot\fresh-retry-result.txt" w
    FileWrite `$R9 "classified=1|legacyPending=`$PCTLegacyUpgradePending|update=`$UpdateMode|installdir=`$INSTDIR|newvalid=`$PCTNewValid`$\n"
    FileClose `$R9
    Call PCTCompleteLegacyMigration
    StrCpy `$PCTRegistrationVerified 1
    Call .onGUIEnd
    FileOpen `$R9 "$harnessRoot\fresh-complete-result.txt" w
    FileWrite `$R9 "failed=`$PCTLegacyCleanupFailed|complete=`$PCTLegacyUpgradeComplete|pending=`$PCTFakeLegacyCleanupMarker|completeMarker=`$PCTFakeLegacyCompleteMarker`$\n"
    FileClose `$R9
  `${Else}
    SetErrorLevel 2
    Quit
  `${EndIf}
SectionEnd
"@
$freshRetryScript = Join-Path $harnessRoot 'fresh-retry-harness.nsi'
[System.IO.File]::WriteAllText($freshRetryScript, $freshRetrySource, [System.Text.UTF8Encoding]::new($false))
$freshRetryCompile = & $nsisCompiler /V2 $freshRetryScript 2>&1
$freshRetryCompileExit = $LASTEXITCODE
$freshRetryCompile | ForEach-Object { Write-Output $_ }
Assert-True ($freshRetryCompileExit -eq 0) "Fresh-run classifier harness compilation failed with exit code $freshRetryCompileExit."
$freshRetryExe = Join-Path $harnessRoot 'fresh-retry-harness.exe'
$freshInstallDir = Join-Path $harnessRoot 'Fresh Retry Install'
$freshExpectedDirectory = [System.IO.Path]::GetFullPath($freshInstallDir).TrimEnd('\').ToLowerInvariant()
New-Item -ItemType Directory -Path $freshInstallDir -Force | Out-Null
$freshUninstaller = Join-Path $freshInstallDir 'uninstall.exe'
$freshApp = Join-Path $freshInstallDir 'anthro-bridge.exe'
[System.IO.File]::WriteAllBytes($freshUninstaller, [byte[]]@(0x4D, 0x5A))
[System.IO.File]::WriteAllBytes($freshApp, [byte[]]@(0x4D, 0x5A))
$appSentinel = Join-Path $freshInstallDir 'install-payload.sentinel'
$userSentinel = Join-Path $harnessRoot 'user-data.sentinel'
[System.IO.File]::WriteAllText($appSentinel, 'installed payload must remain unchanged')
[System.IO.File]::WriteAllText($userSentinel, 'user data must remain unchanged')
$quotedFreshDir = '"' + $freshInstallDir + '"'
$quotedFreshUninstaller = '"' + $freshUninstaller + '"'
$quotedFreshApp = '"' + $freshApp + '"'
$freshFixtureValues = @{
  LegacyInstallLocation = $quotedFreshDir
  LegacyUninstallString = $quotedFreshUninstaller
  LegacyPublisherPath = $quotedFreshDir
  NewInstallLocation = $quotedFreshDir
  NewUninstallString = $quotedFreshUninstaller
  NewDisplayIcon = $quotedFreshApp
}

function New-FreshRetryFixture([string]$Path, [bool]$UninstallPresent, [bool]$PublisherPresent) {
  $fixture = @"
[Migration]
Pending=1
CleanupPending=0
UpgradePath=$freshExpectedDirectory
Complete=0
[LegacyUninstall]
Present=$(if ($UninstallPresent) { '1' } else { '0' })
DisplayName=Anthro Bridge
Publisher=Sohei IDE (soheidon)
DisplayVersion=0.24.0
InstallLocation=$($freshFixtureValues.LegacyInstallLocation)
UninstallString=$($freshFixtureValues.LegacyUninstallString)
MainBinaryName=anthro-bridge.exe
DisplayIcon=
DeleteCalls=0
[LegacyPublisher]
Present=$(if ($PublisherPresent) { '1' } else { '0' })
Path=$($freshFixtureValues.LegacyPublisherPath)
DeleteCalls=0
[PoverelloPublisher]
Path=$($freshFixtureValues.NewInstallLocation)
[PoverelloUninstall]
DisplayName=Poverello Coding Tools
Publisher=Sohei IDE (soheidon)
DisplayVersion=0.24.1
InstallLocation=$($freshFixtureValues.NewInstallLocation)
UninstallString=$($freshFixtureValues.NewUninstallString)
DisplayIcon=$($freshFixtureValues.NewDisplayIcon)
MainBinaryName=anthro-bridge.exe
"@
  [System.IO.File]::WriteAllText($Path, $fixture, [System.Text.UTF8Encoding]::new($false))
}

function Invoke-FreshRetryHarness([string]$Phase, [string]$StatePath, [string]$FailKey = '') {
  $env:PCT_NSIS_TEST_REGISTRY_STATE = $StatePath
  $env:PCT_NSIS_TEST_PHASE = $Phase
  $env:PCT_NSIS_TEST_FAIL_KEY = $FailKey
  try {
    return Start-Process -FilePath $freshRetryExe -WindowStyle Hidden -Wait -PassThru
  } finally {
    Remove-Item Env:PCT_NSIS_TEST_REGISTRY_STATE -ErrorAction SilentlyContinue
    Remove-Item Env:PCT_NSIS_TEST_PHASE -ErrorAction SilentlyContinue
    Remove-Item Env:PCT_NSIS_TEST_FAIL_KEY -ErrorAction SilentlyContinue
  }
}

for ($pass = 1; $pass -le 8; $pass++) {
foreach ($orientation in @(
  @{ Name = 'uninstall-key-removed'; Uninstall = $true; Publisher = $true; FailKey = 'Software\Sohei IDE (soheidon)\Anthro Bridge'; RemovedSection = 'LegacyUninstall'; RemainingSection = 'LegacyPublisher'; RemovedCalls = 1; RemainingCalls = 2 },
  @{ Name = 'publisher-key-removed'; Uninstall = $true; Publisher = $true; FailKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge'; RemovedSection = 'LegacyPublisher'; RemainingSection = 'LegacyUninstall'; RemovedCalls = 1; RemainingCalls = 2 }
)) {
  $statePath = Join-Path $harnessRoot ("fresh-state-{0}-{1}.ini" -f $pass, $orientation.Name)
  New-FreshRetryFixture $statePath $orientation.Uninstall $orientation.Publisher
  $stage = Invoke-FreshRetryHarness 'stage' $statePath $orientation.FailKey
  Assert-True ($stage.ExitCode -eq 0) "Fresh-run stage failed for $($orientation.Name): $($stage.ExitCode)."
  $stageResult = Get-Content -LiteralPath (Join-Path $harnessRoot 'fresh-stage-result.txt') -Raw
  Assert-True ($stageResult -match 'failed=1\|complete=0\|pending=1') "Stage did not persist a pending partial-cleanup state for $($orientation.Name): $stageResult"
  $stageSnapshot = Get-Content -LiteralPath $statePath -Raw
  Assert-True ($stageSnapshot -match '(?m)^\[Migration\][\s\S]*?^Pending=1$' -and $stageSnapshot -match '(?m)^CleanupPending=1$') 'Stage did not durably persist both migration markers.'
  $retry = Invoke-FreshRetryHarness 'retry' $statePath
  Assert-True ($retry.ExitCode -eq 0) "Fresh-process production classification/retry failed for $($orientation.Name): $($retry.ExitCode)."
  $classified = Get-Content -LiteralPath (Join-Path $harnessRoot 'fresh-retry-result.txt') -Raw
  Assert-True ($classified -match 'classified=1\|legacyPending=1\|update=1' -and $classified.Contains($freshExpectedDirectory)) "Fresh process did not classify the persisted residue as the expected in-place migration: $classified"
  $complete = Get-Content -LiteralPath (Join-Path $harnessRoot 'fresh-complete-result.txt') -Raw
  Assert-True ($complete -match 'failed=0\|complete=1\|pending=0\|completeMarker=0') "Fresh-process cleanup did not complete and clear pending markers through the success-gated completion path for $($orientation.Name): $complete"
  $finalState = Get-Content -LiteralPath $statePath -Raw
  Assert-True ($finalState -match '(?m)^Present=0$' -and $finalState -match '(?m)^Complete=$' -and $finalState -match '(?m)^Pending=$' -and $finalState -match '(?m)^CleanupPending=$' -and $finalState -match '(?m)^UpgradePath=$') "Legacy residue, completion, or pending marker state is incorrect after retry for $($orientation.Name)."
  $removedCalls = [regex]::Match($finalState, "(?ms)^\[$($orientation.RemovedSection)\].*?^DeleteCalls=(\d+)$").Groups[1].Value
  $remainingCalls = [regex]::Match($finalState, "(?ms)^\[$($orientation.RemainingSection)\].*?^DeleteCalls=(\d+)$").Groups[1].Value
  Assert-True ($removedCalls -eq [string]$orientation.RemovedCalls -and $remainingCalls -eq [string]$orientation.RemainingCalls) "Retry repeated a delete against the already-absent key or skipped the surviving key ($removedCalls / $remainingCalls)."
  Assert-True ((Get-Content -LiteralPath $appSentinel -Raw) -eq 'installed payload must remain unchanged' -and (Get-Content -LiteralPath $userSentinel -Raw) -eq 'user data must remain unchanged') 'Fresh-run retry changed an application or user-data sentinel.'
  Write-Output "Fresh-process classifier retry ($pass/8, $($orientation.Name)): PASS"
}
}

# Invalid durable identities must be rejected by classification before cleanup
# can alter the fake registry. These negative cases use the same retry process.
$negativeBase = Join-Path $harnessRoot 'fresh-negative-base.ini'
New-FreshRetryFixture $negativeBase $true $true
$negativeStage = Invoke-FreshRetryHarness 'stage' $negativeBase 'Software\Sohei IDE (soheidon)\Anthro Bridge'
Assert-True ($negativeStage.ExitCode -eq 0) 'Could not prepare the durable partial-cleanup fixture for fail-closed cases.'
$negativeCases = @(
  @{ Name = 'missing-pending-marker'; Transform = { param($s) [regex]::Replace($s, '(?m)^Pending=1$', 'Pending=0', 1) } },
  @{ Name = 'changed-pending-path'; Transform = { param($s) $s.Replace("UpgradePath=$freshExpectedDirectory", 'UpgradePath=C:\\unexpected\\path') } },
  @{ Name = 'mismatched-surviving-legacy-registration'; Transform = { param($s) [regex]::Replace($s, '(?ms)(^\[LegacyPublisher\]\r?\nPresent=1\r?\nPath=).*?(\r?\nDeleteCalls=\d+)', '$1"C:\different\install"$2', 1) } },
  @{ Name = 'invalid-poverello-registration'; Transform = { param($s) $s.Replace('DisplayName=Poverello Coding Tools', 'DisplayName=Unexpected Product') } },
  @{ Name = 'missing-cleanup-pending-marker'; Transform = { param($s) $s.Replace('CleanupPending=1', 'CleanupPending=0') } }
)
foreach ($case in $negativeCases) {
  $casePath = Join-Path $harnessRoot ("negative-{0}.ini" -f $case.Name)
  Copy-Item -LiteralPath $negativeBase -Destination $casePath
  $before = [System.IO.File]::ReadAllText($casePath)
  $after = & $case.Transform $before
  Assert-True ($after -ne $before) "Negative fixture transformation did not apply for $($case.Name)."
  [System.IO.File]::WriteAllText($casePath, $after, [System.Text.UTF8Encoding]::new($false))
  $beforeHash = (Get-FileHash -LiteralPath $casePath -Algorithm SHA256).Hash
  Remove-Item -LiteralPath "$casePath.rejected" -Force -ErrorAction SilentlyContinue
  $negativeRun = Invoke-FreshRetryHarness 'retry' $casePath
  Assert-True ($negativeRun.ExitCode -ne 0) "Invalid persisted state '$($case.Name)' was not rejected by the production classifier."
  Assert-True (Test-Path -LiteralPath "$casePath.rejected") "Invalid persisted state '$($case.Name)' did not fail through PCTRejectInstallState."
  Assert-True ((Get-FileHash -LiteralPath $casePath -Algorithm SHA256).Hash -eq $beforeHash) "Classifier mutated fake registry state before rejecting '$($case.Name)'."
  Write-Output "Fresh-process fail-closed classifier ($($case.Name)): PASS"
}
$unreadablePath = Join-Path $harnessRoot 'unreadable-state-does-not-exist.ini'
Remove-Item -LiteralPath $unreadablePath -Force -ErrorAction SilentlyContinue
$unreadableRun = Invoke-FreshRetryHarness 'retry' $unreadablePath
Assert-True ($unreadableRun.ExitCode -ne 0 -and (Test-Path -LiteralPath "$unreadablePath.rejected")) 'Unreadable persisted fake state was not rejected before classification could become retryable.'
Write-Output 'Fresh-process fail-closed classifier (unreadable persisted state): PASS'

# Mutation proof: disabling only the pending-residue acceptance condition in
# a temporary hook copy must reject the otherwise valid persisted fixture.
$mutationState = Join-Path $harnessRoot 'fresh-mutation-state.ini'
New-FreshRetryFixture $mutationState $true $true
$mutationStage = Invoke-FreshRetryHarness 'stage' $mutationState 'Software\Sohei IDE (soheidon)\Anthro Bridge'
Assert-True ($mutationStage.ExitCode -eq 0) 'Could not stage the valid partial-cleanup fixture for classifier mutation proof.'
$hooksText = [System.IO.File]::ReadAllText((Resolve-Path -LiteralPath $HooksPath))
$classifierBranch = '      ${If} $PCTLegacyCleanupMarker == 1'
Assert-True ($hooksText.Contains($classifierBranch)) 'Mutation proof could not locate the classifier pending-residue branch.'
$mutantHooksPath = Join-Path $harnessRoot 'hooks-classifier-mutant.nsh'
$mutantHooks = $hooksText.Replace($classifierBranch, '      ${If} $PCTLegacyCleanupMarker == 0')
[System.IO.File]::WriteAllText($mutantHooksPath, $mutantHooks, [System.Text.UTF8Encoding]::new($false))
$mutantSource = $freshRetrySource.Replace($hooksAbsolute, $mutantHooksPath).Replace('fresh-retry-harness.exe', 'fresh-retry-mutant.exe')
$mutantScriptPath = Join-Path $harnessRoot 'fresh-retry-mutant.nsi'
[System.IO.File]::WriteAllText($mutantScriptPath, $mutantSource, [System.Text.UTF8Encoding]::new($false))
$mutantCompileOutput = & $nsisCompiler /V2 $mutantScriptPath 2>&1
$mutantCompileExit = $LASTEXITCODE
Assert-True ($mutantCompileExit -eq 0) "Mutated classifier harness failed to compile: $mutantCompileOutput"
$mutationStateHash = (Get-FileHash -LiteralPath $mutationState -Algorithm SHA256).Hash
Remove-Item -LiteralPath "$mutationState.rejected" -Force -ErrorAction SilentlyContinue
$env:PCT_NSIS_TEST_REGISTRY_STATE = $mutationState
$env:PCT_NSIS_TEST_PHASE = 'retry'
try {
  $mutantRun = Start-Process -FilePath (Join-Path $harnessRoot 'fresh-retry-mutant.exe') -WindowStyle Hidden -Wait -PassThru
} finally {
  Remove-Item Env:PCT_NSIS_TEST_REGISTRY_STATE -ErrorAction SilentlyContinue
  Remove-Item Env:PCT_NSIS_TEST_PHASE -ErrorAction SilentlyContinue
}
Assert-True ($mutantRun.ExitCode -ne 0 -and (Test-Path -LiteralPath "$mutationState.rejected")) 'Disabling classifier pending-residue acceptance did not fail through the classification rejection path.'
Assert-True ((Get-FileHash -LiteralPath $mutationState -Algorithm SHA256).Hash -eq $mutationStateHash) 'Mutated classifier changed the durable registry fixture before rejection.'
Write-Output 'Classifier mutation proof (pending-residue branch disabled): PASS (specific PCTRejectInstallState marker; no timeout)'

# Run the production uninstaller cleanup routine only against disposable paths.
$uninstallResultPath = Join-Path $harnessRoot 'uninstall-runtime-state.txt'
$uninstallHarnessSource = @"
Unicode true
RequestExecutionLevel user
!define PCT_NSIS_TEST_BACKEND
!include LogicLib.nsh
!include FileFunc.nsh
!include StrFunc.nsh
`${StrCase}
!define PRODUCTNAME "Poverello Coding Tools"
!define MANUFACTURER "Sohei IDE (soheidon)"
!define VERSION "0.24.1"
!define MAINBINARYNAME "anthro-bridge"
!define MANUKEY "Software\`${MANUFACTURER}"
!define MANUPRODUCTKEY "`${MANUKEY}\`${PRODUCTNAME}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\`${PRODUCTNAME}"
Var UpdateMode
Var DeleteAppDataCheckboxState
Var PCTUninstallDeleteRuntimeData
!include "$hooksAbsolute"
!insertmacro PCT_HOOK_FUNCTIONS
Function un.onInit
  StrCpy `$UpdateMode 0
  `${GetOptions} `$CMDLINE "/UPDATE" `$0
  `${IfNot} `${Errors}
    StrCpy `$UpdateMode 1
  `${EndIf}
FunctionEnd
OutFile "$harnessRoot\uninstall-data-setup-harness.exe"
InstallDir "$harnessRoot\unused-install-target"
Section "Harness"
  ReadEnvStr `$PCTTestRegistryStatePath "PCT_NSIS_TEST_REGISTRY_STATE"
  ReadEnvStr `$R8 "PCT_NSIS_TEST_PHASE"
  `${If} `$R8 == "classify-clean"
    StrCpy `$PCTTestRegistryReadFailed 0
    ReadEnvStr `$PCTTestLegacyOrphanPath "PCT_NSIS_TEST_LEGACY_ORPHAN"
    Call PCTClassifyInstallState
    FileOpen `$R9 "$harnessRoot\uninstall-classification.txt" w
    FileWrite `$R9 "classification=clean|newRegistry=`$PCTNewHasRegistry|legacyRegistry=`$PCTLegacyHasRegistry|legacyOrphan=`$PCTLegacyOrphanExecutable`$\n"
    FileClose `$R9
  `${Else}
    WriteUninstaller "$harnessRoot\uninstall-data-generated.exe"
  `${EndIf}
SectionEnd
SilentUnInstall silent
Section "Uninstall"
  ReadEnvStr `$0 "PCT_NSIS_TEST_DELETE_DATA"
  `${If} `$0 == "1"
    StrCpy `$DeleteAppDataCheckboxState 1
  `${Else}
    StrCpy `$DeleteAppDataCheckboxState 0
  `${EndIf}
  !insertmacro NSIS_HOOK_PREUNINSTALL
  ; Match generated Tauri's unconditional uninstall registration deletion.
  !insertmacro PCT_UN_DELETE_REG_KEY "`${UNINSTKEY}"
  ; Match its checkbox branch. PREUNINSTALL deliberately sets the generated
  ; checkbox to zero, so its broad data and publisher-key deletion is skipped.
  `${If} `$DeleteAppDataCheckboxState == 1
  `${AndIf} `$UpdateMode != 1
    !insertmacro PCT_UN_DELETE_REG_KEY "`${MANUPRODUCTKEY}"
  `${EndIf}
  !insertmacro NSIS_HOOK_POSTUNINSTALL
  FileOpen `$8 "$uninstallResultPath" w
  FileWrite `$8 "status=success|checkbox=`$PCTUninstallDeleteRuntimeData|generated-checkbox=`$DeleteAppDataCheckboxState|data-failed=`$PCTUninstallDataFailed|registration-verified=`$PCTUninstallRegistrationVerified|path=`$PCTUninstallDataPath|reparse=`$PCTUninstallReparseFound|no-file-rollback-claimed=1`$\n"
  FileClose `$8
SectionEnd
"@
$uninstallHarnessScript = Join-Path $harnessRoot 'uninstall-data-harness.nsi'
[System.IO.File]::WriteAllText($uninstallHarnessScript, $uninstallHarnessSource, [System.Text.UTF8Encoding]::new($false))
$uninstallCompileOutput = & $nsisCompiler /V2 $uninstallHarnessScript 2>&1
$uninstallCompileExit = $LASTEXITCODE
$uninstallCompileOutput | ForEach-Object { Write-Output $_ }
Assert-True ($uninstallCompileExit -eq 0) "Uninstall-data harness compilation failed with exit code $uninstallCompileExit."
$uninstallSetupExe = Join-Path $harnessRoot 'uninstall-data-setup-harness.exe'
$uninstallExtractedPath = Join-Path $harnessRoot 'uninstall-data-generated.exe'
$extractRun = Start-Process -FilePath $uninstallSetupExe -ArgumentList @('/S') -WindowStyle Hidden -Wait -PassThru
Assert-True ($extractRun.ExitCode -eq 0 -and (Test-Path -LiteralPath $uninstallExtractedPath -PathType Leaf)) 'Test setup harness did not extract its generated uninstaller.'
$uninstallExe = $uninstallExtractedPath
$uninstallFixtureRoot = Join-Path $harnessRoot 'uninstall-fixtures'
$outsideSentinel = Join-Path $uninstallFixtureRoot 'outside-sentinel.txt'
New-Item -ItemType Directory -Path $uninstallFixtureRoot -Force | Out-Null
[System.IO.File]::WriteAllText($outsideSentinel, 'preserve')
$uninstallInstallRoot = Join-Path $uninstallFixtureRoot 'Installed Poverello'
New-Item -ItemType Directory -Path $uninstallInstallRoot -Force | Out-Null
$quotedInstallRoot = '"' + $uninstallInstallRoot + '"'
$quotedUninstaller = '"' + (Join-Path $uninstallInstallRoot 'uninstall.exe') + '"'
$quotedApp = '"' + (Join-Path $uninstallInstallRoot 'anthro-bridge.exe') + '"'
function New-PoverelloUninstallRegistryFixture([string]$Path) {
  $contents = @"
[PoverelloPublisher]
Present=1
Path=$quotedInstallRoot
[PoverelloUninstall]
Present=1
DisplayName=Poverello Coding Tools
DisplayIcon=$quotedApp
DisplayVersion=0.24.1
Publisher=Sohei IDE (soheidon)
InstallLocation=$quotedInstallRoot
UninstallString=$quotedUninstaller
MainBinaryName=anthro-bridge.exe
NoModify=1
NoRepair=0
EstimatedSize=12345
[Migration]
Pending=
CleanupPending=
UpgradePath=
Complete=
[LegacyUninstall]
Present=0
DisplayName=
Publisher=
DisplayVersion=
InstallLocation=
UninstallString=
DisplayIcon=
MainBinaryName=
[LegacyPublisher]
Present=0
Path=
"@
  [System.IO.File]::WriteAllText($Path, $contents, [System.Text.UTF8Encoding]::new($false))
}
function Invoke-PoverelloUninstallHarness([string]$StatePath, [string]$DataPath, [string]$DeleteData, [string]$Arguments = '/S', [string]$FailOperation = '', [string]$FailKey = '', [string]$FailValue = '', [bool]$CaptureInstallerStatus = $false, [string]$FailRegistrationDeleteKey = '') {
  Remove-Item -LiteralPath $uninstallResultPath -Force -ErrorAction SilentlyContinue
  $env:PCT_NSIS_TEST_REGISTRY_STATE = $StatePath
  $env:PCT_NSIS_TEST_DATA_PATH = $DataPath
  $env:PCT_NSIS_TEST_DELETE_DATA = $DeleteData
  $env:PCT_NSIS_TEST_FAIL_OPERATION = $FailOperation
  $env:PCT_NSIS_TEST_FAIL_KEY = $FailKey
  $env:PCT_NSIS_TEST_FAIL_VALUE = $FailValue
  $env:PCT_NSIS_TEST_FAIL_REGISTRATION_DELETE_KEY = $FailRegistrationDeleteKey
  $env:PCT_NSIS_TEST_RESULT_PATH = $uninstallResultPath
  # Tauri's generated update/reinstall path appends _?=$INSTDIR so ExecWait
  # receives the actual uninstaller status instead of the self-copy launcher.
  $effectiveArguments = if ($CaptureInstallerStatus) { "$Arguments _?=$uninstallInstallRoot" } else { $Arguments }
  try { return Start-Process -FilePath $uninstallExe -ArgumentList $effectiveArguments -WindowStyle Hidden -Wait -PassThru }
  finally {
    foreach ($name in @('PCT_NSIS_TEST_REGISTRY_STATE','PCT_NSIS_TEST_DATA_PATH','PCT_NSIS_TEST_DELETE_DATA','PCT_NSIS_TEST_FAIL_OPERATION','PCT_NSIS_TEST_FAIL_KEY','PCT_NSIS_TEST_FAIL_VALUE','PCT_NSIS_TEST_FAIL_REGISTRATION_DELETE_KEY','PCT_NSIS_TEST_RESULT_PATH')) { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
  }
}
function Assert-PoverelloKeysAbsent([string]$StatePath, [string]$Message) {
  $state = [System.IO.File]::ReadAllText($StatePath)
  Assert-True ($state -match '(?m)^Present=0$' -and $state -match '(?ms)^\[PoverelloUninstall\].*?^Present=0$') $Message
}
$oldTestRegistryPath = $env:PCT_NSIS_TEST_REGISTRY_STATE
$oldTestDataPath = $env:PCT_NSIS_TEST_DATA_PATH
$oldDeleteData = $env:PCT_NSIS_TEST_DELETE_DATA
$oldUninstallResultPath = $env:PCT_NSIS_TEST_RESULT_PATH
try {
  $uncheckedRegistry = Join-Path $uninstallFixtureRoot 'unchecked-registry.ini'
  New-PoverelloUninstallRegistryFixture $uncheckedRegistry
  $uncheckedTarget = Join-Path $uninstallFixtureRoot 'unchecked\Poverello'
  New-Item -ItemType Directory -Path $uncheckedTarget -Force | Out-Null
  $uncheckedFile = Join-Path $uncheckedTarget 'config.json'
  [System.IO.File]::WriteAllText($uncheckedFile, 'keep')
  $uncheckedRun = Invoke-PoverelloUninstallHarness $uncheckedRegistry $uncheckedTarget '0'
  $uncheckedState = if(Test-Path -LiteralPath $uninstallResultPath){Get-Content -LiteralPath $uninstallResultPath -Raw}else{''}
  Assert-True ($uncheckedRun.ExitCode -eq 0 -and (Test-Path -LiteralPath $uncheckedFile) -and $uncheckedState.Contains('status=success|checkbox=0|generated-checkbox=0|data-failed=0|registration-verified=1')) "Unchecked delete-app-data behavior did not preserve data and verify registration cleanup (state='$uncheckedState', exit=$($uncheckedRun.ExitCode))."
  Assert-PoverelloKeysAbsent $uncheckedRegistry 'Unchecked uninstall left Poverello registration behind.'

  # Restart a separate installer process and exercise the real production
  # classifier against the persisted fake registry after unchecked uninstall.
  $env:PCT_NSIS_TEST_REGISTRY_STATE = $uncheckedRegistry
  $env:PCT_NSIS_TEST_PHASE = 'classify-clean'
  $env:PCT_NSIS_TEST_LEGACY_ORPHAN = Join-Path $uninstallFixtureRoot 'absent-legacy\anthro-bridge.exe'
  try { $cleanClassify = Start-Process -FilePath $uninstallSetupExe -ArgumentList @('/S') -WindowStyle Hidden -Wait -PassThru }
  finally {
    Remove-Item Env:PCT_NSIS_TEST_REGISTRY_STATE,Env:PCT_NSIS_TEST_PHASE,Env:PCT_NSIS_TEST_LEGACY_ORPHAN -ErrorAction SilentlyContinue
  }
  $cleanClassifyState = Get-Content -LiteralPath (Join-Path $harnessRoot 'uninstall-classification.txt') -Raw
  Assert-True ($cleanClassify.ExitCode -eq 0 -and $cleanClassifyState -match 'classification=clean\|newRegistry=0\|legacyRegistry=0\|legacyOrphan=0' -and -not (Test-Path -LiteralPath "$uncheckedRegistry.rejected")) "A fresh production classifier did not accept the unregistered uninstall state: $cleanClassifyState"

  # Test uninstall when MANUPRODUCTKEY is already absent from the start.
  # The uninstaller must treat "already absent" as success and exit code 0.
  $absentPublisherRegistry = Join-Path $uninstallFixtureRoot 'absent-publisher-registry.ini'
  New-PoverelloUninstallRegistryFixture $absentPublisherRegistry
  $absentPublisherContent = [System.IO.File]::ReadAllText($absentPublisherRegistry)
  $absentPublisherContent = $absentPublisherContent -replace '(?ms)^\[PoverelloPublisher\]\r?\nPresent=1\r?\nPath=.*?(?=\r?\n\[)', "[PoverelloPublisher]`r`nPresent=0`r`nPath="
  [System.IO.File]::WriteAllText($absentPublisherRegistry, $absentPublisherContent, [System.Text.UTF8Encoding]::new($false))
  $absentPublisherRun = Invoke-PoverelloUninstallHarness $absentPublisherRegistry $uncheckedTarget '0'
  $absentPublisherState = if(Test-Path -LiteralPath $uninstallResultPath){Get-Content -LiteralPath $uninstallResultPath -Raw}else{''}
  Assert-True ($absentPublisherRun.ExitCode -eq 0 -and $absentPublisherState.Contains('registration-verified=1')) "Uninstaller must succeed with exit code 0 even when MANUPRODUCTKEY was already absent before uninstall (state='$absentPublisherState', exit=$($absentPublisherRun.ExitCode))."
  Assert-PoverelloKeysAbsent $absentPublisherRegistry 'Uninstall with absent MANUPRODUCTKEY left Poverello registration behind.'

  $absentTarget = Join-Path $uninstallFixtureRoot 'absent\Poverello'
  $env:PCT_NSIS_TEST_DATA_PATH = $absentTarget
  $env:PCT_NSIS_TEST_DELETE_DATA = '1'
  $absentRegistry = Join-Path $uninstallFixtureRoot 'absent-registry.ini'
  New-PoverelloUninstallRegistryFixture $absentRegistry
  $absentRun = Invoke-PoverelloUninstallHarness $absentRegistry $absentTarget '1'
  Assert-True ($absentRun.ExitCode -eq 0 -and -not (Test-Path -LiteralPath $absentTarget)) 'An absent stable data directory must be a successful no-op.'

  $checkedTarget = Join-Path $uninstallFixtureRoot 'checked\Poverello'
  $checkedSiblingRoot = Split-Path -Parent $checkedTarget
  $devSibling = Join-Path $checkedSiblingRoot 'Poverello Dev'
  $legacySibling = Join-Path $checkedSiblingRoot 'Anthro Bridge'
  New-Item -ItemType Directory -Path $devSibling, $legacySibling -Force | Out-Null
  $devSentinel = Join-Path $devSibling 'dev-config.json'
  $legacySentinel = Join-Path $legacySibling 'legacy-config.json'
  [System.IO.File]::WriteAllText($devSentinel, 'preserve-dev')
  [System.IO.File]::WriteAllText($legacySentinel, 'preserve-legacy')
  New-Item -ItemType Directory -Path $checkedTarget -Force | Out-Null
  [System.IO.File]::WriteAllText((Join-Path $checkedTarget 'config.json'), 'delete')
  $checkedNested = Join-Path $checkedTarget 'nested\deeper'
  New-Item -ItemType Directory -Path $checkedNested -Force | Out-Null
  [System.IO.File]::WriteAllText((Join-Path $checkedNested 'nested-config.json'), 'delete-nested')
  $checkedRegistry = Join-Path $uninstallFixtureRoot 'checked-registry.ini'
  New-PoverelloUninstallRegistryFixture $checkedRegistry
  $checkedRun = Invoke-PoverelloUninstallHarness $checkedRegistry $checkedTarget '1'
  $checkedState = if(Test-Path -LiteralPath $uninstallResultPath){Get-Content -LiteralPath $uninstallResultPath -Raw}else{''}
  Assert-True ($checkedRun.ExitCode -eq 0 -and -not (Test-Path -LiteralPath $checkedTarget) -and $checkedState.Contains('status=success|checkbox=1|generated-checkbox=0|data-failed=0|registration-verified=1')) "Checked delete-app-data behavior did not remove stable data and registration (state='$checkedState', exit=$($checkedRun.ExitCode))."
  Assert-PoverelloKeysAbsent $checkedRegistry 'Checked uninstall left Poverello registration behind.'
  Assert-True ((Test-Path -LiteralPath $devSentinel) -and (Test-Path -LiteralPath $legacySentinel)) 'Stable uninstall modified Poverello Dev or the legacy Anthro Bridge data directory.'

  $reparseTarget = Join-Path $uninstallFixtureRoot 'reparse\Poverello'
  $externalTarget = Join-Path $uninstallFixtureRoot 'external'
  New-Item -ItemType Directory -Path $reparseTarget, $externalTarget -Force | Out-Null
  $externalSentinel = Join-Path $externalTarget 'must-survive.txt'
  [System.IO.File]::WriteAllText($externalSentinel, 'preserve')
  $junctionCreated = $false
  try {
    New-Item -ItemType Junction -Path (Join-Path $reparseTarget 'external-link') -Target $externalTarget -ErrorAction Stop | Out-Null
    $junctionCreated = $true
  }
  catch {
    Write-Output "Reparse-point deletion fixture: SKIP (junction creation unavailable: $($_.Exception.Message))"
  }
  if ($junctionCreated) {
    if (Test-Path -LiteralPath $uninstallResultPath) { Remove-Item -LiteralPath $uninstallResultPath -Force }
    $reparseRegistry = Join-Path $uninstallFixtureRoot 'reparse-registry.ini'
    New-PoverelloUninstallRegistryFixture $reparseRegistry
    $reparseRun = Invoke-PoverelloUninstallHarness $reparseRegistry $reparseTarget '1' '/S' '' '' '' $true
    $reparseState = if(Test-Path -LiteralPath $uninstallResultPath){Get-Content -LiteralPath $uninstallResultPath -Raw}else{''}
    Assert-True ((Test-Path -LiteralPath $reparseTarget) -and (Test-Path -LiteralPath $externalSentinel) -and $reparseState.Contains('cleanup-failed=1|reparse=1') -and $reparseState.Contains('registration-verified=1|no-file-rollback-claimed=1') -and $reparseRun.ExitCode -ne 0) "A reparse point must preserve external data, clean registration, and truthfully fail without claiming file rollback (state='$reparseState', exit=$($reparseRun.ExitCode))."
  }

  # A deterministic pre-delete failure must preserve data, still remove both
  # registration keys, and report a non-zero truthful failure.
  $failedDataRegistry = Join-Path $uninstallFixtureRoot 'data-failure-registry.ini'
  New-PoverelloUninstallRegistryFixture $failedDataRegistry
  $failedDataTarget = Join-Path $uninstallFixtureRoot 'data-failure\Poverello'
  New-Item -ItemType Directory -Path $failedDataTarget -Force | Out-Null
  $failedDataSentinel = Join-Path $failedDataTarget 'keep-on-failure.txt'
  [System.IO.File]::WriteAllText($failedDataSentinel, 'preserve-on-failure')
  $failedDataRun = Invoke-PoverelloUninstallHarness $failedDataRegistry $failedDataTarget '1' '/S' 'data-delete' '' '' $true
  $failedDataState = Get-Content -LiteralPath $uninstallResultPath -Raw
  Assert-True ($failedDataRun.ExitCode -ne 0 -and (Test-Path -LiteralPath $failedDataSentinel) -and $failedDataState.Contains('status=failed|reason=data-cleanup') -and $failedDataState.Contains('registration-verified=1|no-file-rollback-claimed=1')) "Data cleanup failure was not reported after registration cleanup (state='$failedDataState', exit=$($failedDataRun.ExitCode))."
  Assert-PoverelloKeysAbsent $failedDataRegistry 'Data cleanup failure skipped product registration cleanup.'

  # If both data and registry cleanup fail, the message/state must not claim
  # that registration was removed.
  $combinedFailureRegistry = Join-Path $uninstallFixtureRoot 'combined-failure-registry.ini'
  New-PoverelloUninstallRegistryFixture $combinedFailureRegistry
  $combinedFailureTarget = Join-Path $uninstallFixtureRoot 'combined-failure\Poverello'
  New-Item -ItemType Directory -Path $combinedFailureTarget -Force | Out-Null
  $combinedSentinel = Join-Path $combinedFailureTarget 'preserve.txt'
  [System.IO.File]::WriteAllText($combinedSentinel, 'preserve')
  $combinedFailureRun = Invoke-PoverelloUninstallHarness $combinedFailureRegistry $combinedFailureTarget '1' '/S' 'data-delete' '' '' $true 'Software\Sohei IDE (soheidon)\Poverello Coding Tools'
  $combinedFailureState = Get-Content -LiteralPath $uninstallResultPath -Raw
  $combinedFailureSnapshot = [System.IO.File]::ReadAllText($combinedFailureRegistry)
  Assert-True ($combinedFailureRun.ExitCode -ne 0 -and (Test-Path -LiteralPath $combinedSentinel) -and $combinedFailureState.Contains('registration-verified=0') -and $combinedFailureSnapshot -match '(?m)^Present=1$') "Combined data/registration failure was falsely reported as complete cleanup (state='$combinedFailureState', exit=$($combinedFailureRun.ExitCode))."

  # Native-operation-equivalent registry deletion failure must be sticky and
  # visible even if the generated unconditional uninstall-key delete worked.
  $failedRegistry = Join-Path $uninstallFixtureRoot 'registry-failure.ini'
  New-PoverelloUninstallRegistryFixture $failedRegistry
  $failedRegistryRun = Invoke-PoverelloUninstallHarness $failedRegistry $uncheckedTarget '0' '/S' 'registration-delete' 'Software\Sohei IDE (soheidon)\Poverello Coding Tools' '' $true
  $failedRegistryState = Get-Content -LiteralPath $uninstallResultPath -Raw
  Assert-True ($failedRegistryRun.ExitCode -ne 0 -and $failedRegistryState.Contains('status=failed|reason=registration-cleanup|registration-verified=0')) "Registration delete failure was not reported as non-zero (state='$failedRegistryState', exit=$($failedRegistryRun.ExitCode))."
  $failedRegistrySnapshot = [System.IO.File]::ReadAllText($failedRegistry)
  Assert-True ($failedRegistrySnapshot -match '(?m)^Present=1$' -and $failedRegistrySnapshot -match '(?ms)^\[PoverelloUninstall\].*?^Present=0$') 'Registration failure fixture did not preserve the failed publisher key while recording generated uninstall-key deletion.'

  # UPDATE reproduces the generated unconditional uninstall-key deletion.
  $updateRegistry = Join-Path $uninstallFixtureRoot 'update-registry.ini'
  New-PoverelloUninstallRegistryFixture $updateRegistry
  $updateData = Join-Path $uninstallFixtureRoot 'update-data\Poverello'
  New-Item -ItemType Directory -Path $updateData -Force | Out-Null
  $updateDataSentinel = Join-Path $updateData 'keep.txt'
  [System.IO.File]::WriteAllText($updateDataSentinel, 'preserve-update')
  $updateRun = Invoke-PoverelloUninstallHarness $updateRegistry $updateData '0' '/S /UPDATE'
  $updateState = Get-Content -LiteralPath $uninstallResultPath -Raw
  $restoredState = [System.IO.File]::ReadAllText($updateRegistry)
  Assert-True ($updateRun.ExitCode -eq 0 -and $updateState.Contains('status=success|checkbox=0|generated-checkbox=0|data-failed=0|registration-verified=1') -and (Test-Path -LiteralPath $updateDataSentinel)) "UPDATE did not restore and verify registration while preserving data (state='$updateState', exit=$($updateRun.ExitCode))."
  Assert-True ($restoredState -match '(?ms)^\[PoverelloPublisher\]\r?\nPresent=1\r?\nPath=.*?\r?\n\[PoverelloUninstall\]\r?\nPresent=1' -and $restoredState.Contains('DisplayName=Poverello Coding Tools') -and $restoredState.Contains("UninstallString=$quotedUninstaller")) 'UPDATE did not restore the saved registration values.'

  $restoreFailRegistry = Join-Path $uninstallFixtureRoot 'update-restore-failure.ini'
  New-PoverelloUninstallRegistryFixture $restoreFailRegistry
  $restoreFailRun = Invoke-PoverelloUninstallHarness $restoreFailRegistry $updateData '0' '/S /UPDATE' 'update-restore-write' 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Poverello Coding Tools' 'DisplayName' $true
  $restoreFailState = Get-Content -LiteralPath $uninstallResultPath -Raw
  Assert-True ($restoreFailRun.ExitCode -ne 0 -and $restoreFailState.Contains('status=failed|reason=update-registration-restore|fallback-cleanup-verified=1|no-file-rollback-claimed=1')) "UPDATE restore failure was not explicitly recoverable and non-zero (state='$restoreFailState', exit=$($restoreFailRun.ExitCode))."
  Assert-PoverelloKeysAbsent $restoreFailRegistry 'Failed UPDATE restoration did not run the safe registration cleanup fallback.'

  $restoreAndFallbackFailRegistry = Join-Path $uninstallFixtureRoot 'update-restore-and-fallback-failure.ini'
  New-PoverelloUninstallRegistryFixture $restoreAndFallbackFailRegistry
  $restoreAndFallbackFailRun = Invoke-PoverelloUninstallHarness $restoreAndFallbackFailRegistry $updateData '0' '/S /UPDATE' 'update-restore-write' 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Poverello Coding Tools' 'DisplayName' $true 'Software\Sohei IDE (soheidon)\Poverello Coding Tools'
  $restoreAndFallbackFailState = Get-Content -LiteralPath $uninstallResultPath -Raw
  $restoreAndFallbackFailSnapshot = [System.IO.File]::ReadAllText($restoreAndFallbackFailRegistry)
  Assert-True ($restoreAndFallbackFailRun.ExitCode -ne 0 -and $restoreAndFallbackFailState.Contains('status=failed|reason=update-registration-restore|fallback-cleanup-verified=0|no-file-rollback-claimed=1') -and $restoreAndFallbackFailSnapshot -match '(?m)^Present=1$') "UPDATE restore and fallback-cleanup failure was not explicitly diagnosed (state='$restoreAndFallbackFailState', exit=$($restoreAndFallbackFailRun.ExitCode))."
  Assert-True ((Get-Content -LiteralPath $outsideSentinel -Raw) -eq 'preserve') 'Uninstaller cleanup modified a sibling path outside the selected target.'
}
finally {
  if ($null -eq $oldTestRegistryPath) { Remove-Item Env:PCT_NSIS_TEST_REGISTRY_STATE -ErrorAction SilentlyContinue }
  else { $env:PCT_NSIS_TEST_REGISTRY_STATE = $oldTestRegistryPath }
  if ($null -eq $oldTestDataPath) { Remove-Item Env:PCT_NSIS_TEST_DATA_PATH -ErrorAction SilentlyContinue }
  else { $env:PCT_NSIS_TEST_DATA_PATH = $oldTestDataPath }
  if ($null -eq $oldDeleteData) { Remove-Item Env:PCT_NSIS_TEST_DELETE_DATA -ErrorAction SilentlyContinue }
  else { $env:PCT_NSIS_TEST_DELETE_DATA = $oldDeleteData }
  if ($null -eq $oldUninstallResultPath) { Remove-Item Env:PCT_NSIS_TEST_RESULT_PATH -ErrorAction SilentlyContinue }
  else { $env:PCT_NSIS_TEST_RESULT_PATH = $oldUninstallResultPath }
}
Write-Output "Production uninstaller data-path behavior harness: PASS ($uninstallExe)"

Write-Output 'NSIS static contract checks: PASS'
Write-Output "Installer script: $((Resolve-Path -LiteralPath $InstallerScript).Path)"
Write-Output "Hook source: $((Resolve-Path -LiteralPath $HooksPath).Path)"
