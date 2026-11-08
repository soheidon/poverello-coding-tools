# Test suite for build-windows-nsis.ps1 guards, staging isolation, and updater signing handoff (INS-005)
param()

$ErrorActionPreference = 'Stop'
$scriptRoot = $PSScriptRoot
$guiRoot = Split-Path -Parent $scriptRoot
$repoRoot = Split-Path -Parent $guiRoot
$helperScript = Join-Path $scriptRoot 'build-windows-nsis.ps1'
$tauriRoot = Join-Path $guiRoot 'src-tauri'
$verifierScript = Join-Path $scriptRoot 'verify-updater-signature.mjs'

$testsPassed = 0
$testsFailed = 0

function Run-Test([string]$Name, [scriptblock]$Body) {
  Write-Host "[TEST] $Name ... " -NoNewline
  try {
    & $Body
    Write-Host "PASS" -ForegroundColor Green
    $script:testsPassed++
  } catch {
    Write-Host "FAIL: $($_ | Out-String)" -ForegroundColor Red
    $script:testsFailed++
  }
}

function Assert-Throws([scriptblock]$Script, [string]$Pattern, [string]$Context = "") {
  $threw = $false
  $actualMessage = ""
  try {
    $output = & $Script 2>&1
    if ($LASTEXITCODE -ne 0) {
      $threw = $true
      $actualMessage = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    } else {
      $actualMessage = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    }
  } catch {
    $threw = $true
    $actualMessage = $_.Exception.Message
  }
  if (-not $threw) {
    throw "Expected exception matching '$Pattern', but no exception was thrown. ($Context)"
  }
  if ($actualMessage -notmatch $Pattern) {
    throw "Exception message '$actualMessage' did not match expected pattern '$Pattern'. ($Context)"
  }
}

function Invoke-StagingHelper([scriptblock]$Command) {
  $previousChannel = [Environment]::GetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'Process')
  $previousMode = [Environment]::GetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', 'Process')
  try {
    [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'dev', 'Process')
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', 'staging', 'Process')
    & $Command
  } finally {
    [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', $previousChannel, 'Process')
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', $previousMode, 'Process')
  }
}

# Preserve ambient environment variables
$origPublicKey = [Environment]::GetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', 'Process')
$origPrivateKey = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'Process')
$origPrivateKeyPath = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', 'Process')
$origPassword = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'Process')
$origChannel = [Environment]::GetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'Process')
$origCargoTarget = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')

# Valid sample public key text for testing preflight
$samplePublicKeyText = "untrusted comment: minisign public key: A49EF4746FA6E6D7`nRWTX5qZvdPSepDGWg2bz8n6ArQnBMwgQK1geVosorcbLzcunD+KJ+cYe"
$validBase64PublicKey = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($samplePublicKeyText))

try {
  # =========================================================================
  # Suite 1: -RequireUpdaterSigning fail-closed preflight tests
  # =========================================================================

  Run-Test "RequireUpdaterSigning fails when -ReuseGeneratedBundle is specified (P1-1)" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
    $tempFile = [System.IO.Path]::GetTempFileName()
    try {
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $tempFile, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'dummy', 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

      Assert-Throws {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning -ReuseGeneratedBundle
      } 'does not permit -ReuseGeneratedBundle'
    } finally {
      if (Test-Path -LiteralPath $tempFile) { Remove-Item -LiteralPath $tempFile -Force }
    }
  }

  Run-Test "RequireUpdaterSigning fails when ANTHRO_BRIDGE_CHANNEL is missing or empty (P1-1)" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
    $tempFile = [System.IO.Path]::GetTempFileName()
    try {
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $tempFile, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'dummy', 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', $null, 'Process')

      Assert-Throws {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
      } "requires ANTHRO_BRIDGE_CHANNEL='stable'"
    } finally {
      if (Test-Path -LiteralPath $tempFile) { Remove-Item -LiteralPath $tempFile -Force }
    }
  }

  Run-Test "RequireUpdaterSigning fails when ANTHRO_BRIDGE_CHANNEL is dev" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
    $tempFile = [System.IO.Path]::GetTempFileName()
    try {
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $tempFile, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'somepassword', 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'dev', 'Process')

      Assert-Throws {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
      } "requires ANTHRO_BRIDGE_CHANNEL='stable'"
    } finally {
      if (Test-Path -LiteralPath $tempFile) { Remove-Item -LiteralPath $tempFile -Force }
    }
  }

  Run-Test "RequireUpdaterSigning fails when PCT_UPDATER_PUBLIC_KEY is missing" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $null, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'dummy', 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $null, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'dummy', 'Process')
    [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

    Assert-Throws {
      & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
    } 'PCT_UPDATER_PUBLIC_KEY'
  }

  Run-Test "RequireUpdaterSigning fails when public key format is invalid" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', 'not-valid-base64-or-minisign', 'Process')
    $tempFile = [System.IO.Path]::GetTempFileName()
    try {
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $tempFile, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'dummy', 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

      Assert-Throws {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
      } 'PCT_UPDATER_PUBLIC_KEY must be base64-encoded|PCT_UPDATER_PUBLIC_KEY is not valid'
    } finally {
      if (Test-Path -LiteralPath $tempFile) { Remove-Item -LiteralPath $tempFile -Force }
    }
  }

  Run-Test "RequireUpdaterSigning fails when private key path is missing" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $null, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'dummy', 'Process')
    [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

    Assert-Throws {
      & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
    } 'private key'
  }

  Run-Test "RequireUpdaterSigning fails when private key path does not exist on disk" {
    $nonExistentPath = Join-Path ([System.IO.Path]::GetTempPath()) "non-existent-key-$([guid]::NewGuid().ToString('N')).key"
    try {
      [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $nonExistentPath, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'dummy', 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

      Assert-Throws {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
      } 'an accessible private key file path|Private key file does not exist'
    } finally {
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $null, 'Process')
    }
  }

  Run-Test "RequireUpdaterSigning fails when password environment variable is missing" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
    $tempFile = [System.IO.Path]::GetTempFileName()
    try {
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $tempFile, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', $null, 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

      Assert-Throws {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -RequireUpdaterSigning
      } 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
    } finally {
      if (Test-Path -LiteralPath $tempFile) { Remove-Item -LiteralPath $tempFile -Force }
    }
  }

  # =========================================================================
  # Suite 2: Staging isolation and overlay validation tests (P1-2, P1-3, P2-1)
  # =========================================================================

  Run-Test "BuildOutputRoot without TauriConfigOverlay is rejected (P1-2)" {
    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    try {
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -BuildOutputRoot $tempDir }
      } 'Staging mode requires -TauriConfigOverlay'
    } finally {
      if (Test-Path -LiteralPath $tempDir) { Remove-Item -LiteralPath $tempDir -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay without BuildOutputRoot is rejected (P1-2)" {
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay }
      } 'Staging mode requires -BuildOutputRoot'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
    }
  }

  Run-Test "BuildOutputRoot inside repository directory is rejected (P2-1)" {
    $insideRepoDir = Join-Path $repoRoot "temp-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $insideRepoDir }
      } 'must be located outside the repository directory'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $insideRepoDir) { Remove-Item -LiteralPath $insideRepoDir -Recurse -Force }
    }
  }

  Run-Test "BuildOutputRoot as relative path is rejected" {
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot "relative\output\dir" }
      } 'must be an absolute path'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
    }
  }

  Run-Test "BuildOutputRoot overlapping with default target root is rejected" {
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    $overlapPath = Join-Path $tauriRoot 'target\release\custom-output'
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $overlapPath }
      } 'must be located outside the repository directory|must not overlap'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
    }
  }

  Run-Test "BuildOutputRoot pointing to a non-empty existing directory is rejected" {
    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
    $dummyFile = Join-Path $tempDir 'existing-file.txt'
    Set-Content -LiteralPath $dummyFile -Value 'content'
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $tempDir }
      } 'exists and is not empty'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $tempDir) { Remove-Item -LiteralPath $tempDir -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay with unauthorized top-level key is rejected (P1-3)" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      # Attempting to tamper with 'version' or other top-level keys
      Set-Content -LiteralPath $tempOverlay -Value '{"version":"9.9.9","plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'unauthorized top-level key'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay with non-lowercase top-level key is rejected (P2-2)" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      # Mixed case 'Plugins' must be rejected strictly (case-sensitive check)
      Set-Content -LiteralPath $tempOverlay -Value '{"Plugins":{"updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'unauthorized top-level key'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay with non-lowercase plugin key is rejected (P2-2)" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      # Mixed case 'Updater' must be rejected strictly
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"Updater":{"endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'unauthorized key'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay pointing to non-existent file is rejected" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $nonExistentOverlay = Join-Path ([System.IO.Path]::GetTempPath()) "non-existent-overlay-$([guid]::NewGuid().ToString('N')).json"
    try {
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $nonExistentOverlay -BuildOutputRoot $stagingRoot }
      } 'file does not exist'
    } finally {
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay with invalid JSON is rejected" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{ not valid json }' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'not valid JSON'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay with HTTP endpoint is rejected" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["http://insecure.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'must use HTTPS'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay with default production endpoint is rejected" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://github.com/soheidon/poverello-coding-tools/releases/latest/download/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'must not point to the default production endpoint'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay rejects GitHub Pages as a staging host" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://poverello.github.io/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'must not use GitHub production hosting'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay rejects GitHub release download without pct-updater-smoke- prefix (INS-007)" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://github.com/soheidon/poverello-coding-tools/releases/download/v0.27.1/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'must not use GitHub production hosting'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay rejects GitHub release download for different repository (INS-007)" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://github.com/other-org/other-repo/releases/download/pct-updater-smoke-v0.27.1-test/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'must not use GitHub production hosting'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay rejects uppercase GitHub release download tag prefix (INS-007)" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"endpoints":["https://github.com/soheidon/poverello-coding-tools/releases/download/PCT-UPDATER-SMOKE-v0.27.1-test/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'must not use GitHub production hosting'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay accepts valid GitHub staging smoke prerelease endpoint under INS-007" {
    $testDir = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-smoke-$([guid]::NewGuid().ToString('N'))"
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-out-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $testDir -Force | Out-Null
    $tempOverlay = Join-Path $testDir 'staging-overlay.json'
    try {
      $keyPath = Join-Path $testDir 'test-key.key'
      $pubKeyPath = "$keyPath.pub"
      $testPassword = 'TestPassword123!'

      Push-Location $guiRoot
      try {
        & npm run tauri -- signer generate -w $keyPath -p $testPassword | Out-Null
      } finally {
        Pop-Location
      }

      $base64PubKey = (Get-Content -LiteralPath $pubKeyPath -Raw).Trim()
      Set-Content -LiteralPath $tempOverlay -Value ('{"plugins":{"updater":{"pubkey":"' + $base64PubKey + '","endpoints":["https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-test/latest.json"]}}}') -Encoding UTF8

      $prevEAP = $ErrorActionPreference
      $exitCode = 0
      try {
        $ErrorActionPreference = 'Continue'
        $output = & powershell -NoProfile -ExecutionPolicy Bypass -Command "& {
          [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', '$base64PubKey', 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', `$null, 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', '$keyPath', 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', '$testPassword', 'Process')
          [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'dev', 'Process')
          [Environment]::SetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', 'staging', 'Process')
          & '$helperScript' -BuildMode Staging -RequireUpdaterSigning -PreflightOnly -TauriConfigOverlay '$tempOverlay' -BuildOutputRoot '$stagingRoot'
        }" 2>&1
        $exitCode = $LASTEXITCODE
        $outText = ($output | ForEach-Object { $_.ToString() }) -join "`n"
      } finally {
        $ErrorActionPreference = $prevEAP
      }

      if ($exitCode -ne 0) {
        throw "Staging helper preflight failed with exit code ${exitCode}: $outText"
      }
      if ($outText -notmatch '\[Staging\] updater signing preflight passed') {
        throw "Staging helper did not output expected preflight-only pass confirmation: $outText"
      }
    } finally {
      if (Test-Path -LiteralPath $testDir) { Remove-Item -LiteralPath $testDir -Recurse -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "TauriConfigOverlay requires the runtime and Tauri public keys to match" {
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-$([guid]::NewGuid().ToString('N'))"
    $tempOverlay = [System.IO.Path]::GetTempFileName()
    try {
      [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $validBase64PublicKey, 'Process')
      Set-Content -LiteralPath $tempOverlay -Value '{"plugins":{"updater":{"pubkey":"different-test-key","endpoints":["https://staging.example.com/latest.json"]}}}' -Encoding UTF8
      Assert-Throws {
        Invoke-StagingHelper { & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -BuildMode Staging -TauriConfigOverlay $tempOverlay -BuildOutputRoot $stagingRoot }
      } 'public key must exactly match PCT_UPDATER_PUBLIC_KEY'
    } finally {
      if (Test-Path -LiteralPath $tempOverlay) { Remove-Item -LiteralPath $tempOverlay -Force }
      if (Test-Path -LiteralPath $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
    }
  }

  Run-Test "Stable helper defaults to fail-closed signing preflight" {
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $null, 'Process')
    [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')
    [Environment]::SetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', 'stable', 'Process')
    Assert-Throws {
      & powershell -NoProfile -ExecutionPolicy Bypass -File $helperScript -PreflightOnly
    } 'requires PCT_UPDATER_PUBLIC_KEY'
  }

  # =========================================================================
  # Suite 3: Build helper integration & ephemeral signing tests (P2-2)
  # =========================================================================

  Run-Test "Build helper preflight passes when all -RequireUpdaterSigning requirements are met (P2-2)" {
    $testDir = Join-Path ([System.IO.Path]::GetTempPath()) "test-helper-preflight-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $testDir -Force | Out-Null
    try {
      $keyPath = Join-Path $testDir 'test-key.key'
      $pubKeyPath = "$keyPath.pub"
      $testPassword = 'TestPassword123!'

      # Generate ephemeral test key pair
      Push-Location $guiRoot
      try {
        & npm run tauri -- signer generate -w $keyPath -p $testPassword | Out-Null
      } finally {
        Pop-Location
      }

      $base64PubKey = (Get-Content -LiteralPath $pubKeyPath -Raw).Trim()

      # Set up valid environment for -RequireUpdaterSigning, and pass the key path via TAURI_SIGNING_PRIVATE_KEY
      # to test that the helper correctly resolves the path
      [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $base64PubKey, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $keyPath, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $null, 'Process')
      [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', $testPassword, 'Process')
      [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')

      # Run the helper script with -PreflightOnly: MUST pass all preflight checks,
      # MUST resolve the key path from TAURI_SIGNING_PRIVATE_KEY, and MUST exit 0 WITHOUT starting any build.
      $prevEAP = $ErrorActionPreference
      $exitCode = 0
      try {
        $ErrorActionPreference = 'Continue'
        $output = & powershell -NoProfile -ExecutionPolicy Bypass -Command "& {
          [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', '$base64PubKey', 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', '$keyPath', 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', '$testPassword', 'Process')
          [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'stable', 'Process')
          & '$helperScript' -RequireUpdaterSigning -PreflightOnly
        }" 2>&1
        $exitCode = $LASTEXITCODE
        $outText = ($output | ForEach-Object { $_.ToString() }) -join "`n"
      } finally {
        $ErrorActionPreference = $prevEAP
      }

      if ($exitCode -ne 0) {
        throw "Helper preflight failed with exit code ${exitCode}: $outText"
      }
      if ($outText -notmatch '\[Stable\] updater signing preflight passed') {
        throw "Helper did not output expected preflight-only pass confirmation: $outText"
      }
    } finally {
      if (Test-Path -LiteralPath $testDir) { Remove-Item -LiteralPath $testDir -Recurse -Force }
    }
  }

  Run-Test "Build helper preflight passes in staging mode with overlay and output root (P1-1, P1-2, P2-2)" {
    $testDir = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-preflight-$([guid]::NewGuid().ToString('N'))"
    $stagingOut = Join-Path ([System.IO.Path]::GetTempPath()) "test-staging-out-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $testDir -Force | Out-Null
    $tempOverlay = Join-Path $testDir 'staging-overlay.json'
    try {
      $keyPath = Join-Path $testDir 'test-key.key'
      $pubKeyPath = "$keyPath.pub"
      $testPassword = 'TestPassword123!'

      Push-Location $guiRoot
      try {
        & npm run tauri -- signer generate -w $keyPath -p $testPassword | Out-Null
      } finally {
        Pop-Location
      }

      $base64PubKey = (Get-Content -LiteralPath $pubKeyPath -Raw).Trim()
      Set-Content -LiteralPath $tempOverlay -Value ('{"plugins":{"updater":{"pubkey":"' + $base64PubKey + '","endpoints":["https://staging.poverello.dev/latest.json"]}}}') -Encoding UTF8

      $prevEAP = $ErrorActionPreference
      $exitCode = 0
      try {
        $ErrorActionPreference = 'Continue'
        $output = & powershell -NoProfile -ExecutionPolicy Bypass -Command "& {
          [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', '$base64PubKey', 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', '$keyPath', 'Process')
          [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', '$testPassword', 'Process')
          [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', 'dev', 'Process')
          [Environment]::SetEnvironmentVariable('PCT_UPDATER_BUILD_MODE', 'staging', 'Process')
          & '$helperScript' -BuildMode Staging -RequireUpdaterSigning -PreflightOnly -TauriConfigOverlay '$tempOverlay' -BuildOutputRoot '$stagingOut'
        }" 2>&1
        $exitCode = $LASTEXITCODE
        $outText = ($output | ForEach-Object { $_.ToString() }) -join "`n"
      } finally {
        $ErrorActionPreference = $prevEAP
      }

      if ($exitCode -ne 0) {
        throw "Staging helper preflight failed with exit code ${exitCode}: $outText"
      }
      if ($outText -notmatch '\[Staging\] updater signing preflight passed') {
        throw "Staging helper did not output expected preflight-only pass confirmation: $outText"
      }
    } finally {
      if (Test-Path -LiteralPath $testDir) { Remove-Item -LiteralPath $testDir -Recurse -Force }
      if (Test-Path -LiteralPath $stagingOut) { Remove-Item -LiteralPath $stagingOut -Recurse -Force }
    }
  }

  Run-Test "Ephemeral test key generation, signer path env handoff, and signature verification" {
    $testDir = Join-Path ([System.IO.Path]::GetTempPath()) "test-updater-sign-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $testDir -Force | Out-Null
    try {
      $keyPath = Join-Path $testDir 'test-key.key'
      $pubKeyPath = "$keyPath.pub"
      $testPassword = 'TestPassword123!'
      $testAppVersion = '0.27.0'

      # 1. Generate ephemeral key pair
      Push-Location $guiRoot
      try {
        $genOutput = & npm run tauri -- signer generate -w $keyPath -p $testPassword 2>&1
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $keyPath) -or -not (Test-Path -LiteralPath $pubKeyPath)) {
          throw "Failed to generate ephemeral test key pair via Tauri CLI: $genOutput"
        }
      } finally {
        Pop-Location
      }

      # Tauri CLI writes the Base64-encoded Minisign public key text into the .pub file
      $rawPubContent = Get-Content -LiteralPath $pubKeyPath -Raw
      $base64PubKey = $rawPubContent.Trim()

      # 2. Create dummy installer payload
      $payloadPath = Join-Path $testDir 'dummy-installer.exe'
      [System.IO.File]::WriteAllBytes($payloadPath, [System.Text.Encoding]::UTF8.GetBytes("POVERELLO_TEST_INSTALLER_PAYLOAD_BYTES"))
      $sigPath = "$payloadPath.sig"

      # 3. Simulate build helper handoff:
      # helper sets TAURI_SIGNING_PRIVATE_KEY_PATH to $keyPath, clears TAURI_SIGNING_PRIVATE_KEY,
      # and invokes tauri signer sign --app-version
      $tauriCli = Join-Path $guiRoot 'node_modules\.bin\tauri.cmd'
      Push-Location $guiRoot
      $prevKey = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'Process')
      $prevPath = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', 'Process')
      $prevPass = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'Process')
      try {
        [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $null, 'Process')
        Remove-Item env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
        [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $keyPath, 'Process')
        $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $keyPath
        [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', $testPassword, 'Process')
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $testPassword

        $signOut = & $tauriCli signer sign --app-version $testAppVersion $payloadPath 2>&1
        $signExit = $LASTEXITCODE
        if ($signExit -ne 0 -or -not (Test-Path -LiteralPath $sigPath)) {
          throw "Tauri CLI signing failed with exit code ${signExit}: $signOut"
        }
      } finally {
        Pop-Location
        [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $prevKey, 'Process')
        [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $prevPath, 'Process')
        [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', $prevPass, 'Process')
      }

      # 4. Offline signature verification via node verify-updater-signature.mjs
      $prevPubKeyEnv = [Environment]::GetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', 'Process')
      try {
        $env:PCT_UPDATER_PUBLIC_KEY = $base64PubKey
        [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $base64PubKey, 'Process')
        $prevEAP = $ErrorActionPreference
        try {
          $ErrorActionPreference = 'Continue'
          $verifyOut = & node $verifierScript $payloadPath $sigPath '-' $testAppVersion 2>&1
          $verifyExit = $LASTEXITCODE
        } finally {
          $ErrorActionPreference = $prevEAP
        }
        if ($verifyExit -ne 0) {
          throw "Offline verification of genuine signature failed (exit code $verifyExit): $verifyOut"
        }

        # 5. Tampered payload must be rejected
        $tamperedPayload = Join-Path $testDir 'tampered-installer.exe'
        [System.IO.File]::WriteAllBytes($tamperedPayload, [System.Text.Encoding]::UTF8.GetBytes("TAMPERED_BYTES"))
        try {
          $ErrorActionPreference = 'Continue'
          $tamperOut = & node $verifierScript $tamperedPayload $sigPath '-' $testAppVersion 2>&1
          $tamperExit = $LASTEXITCODE
        } finally {
          $ErrorActionPreference = $prevEAP
        }
        if ($tamperExit -eq 0) {
          throw "Offline verifier accepted tampered payload!"
        }

        # 6. Version mismatch must be rejected
        try {
          $ErrorActionPreference = 'Continue'
          $mismatchOut = & node $verifierScript $payloadPath $sigPath '-' '0.28.0' 2>&1
          $mismatchExit = $LASTEXITCODE
        } finally {
          $ErrorActionPreference = $prevEAP
        }
        if ($mismatchExit -eq 0) {
          throw "Offline verifier accepted mismatched version!"
        }
      } finally {
        [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $prevPubKeyEnv, 'Process')
        $env:PCT_UPDATER_PUBLIC_KEY = $prevPubKeyEnv
      }
    } finally {
      # Safe cleanup of ephemeral test keys and test directory
      if (Test-Path -LiteralPath $testDir) {
        Remove-Item -LiteralPath $testDir -Recurse -Force
      }
    }
  }

} finally {
  # Restore ambient environment variables
  [Environment]::SetEnvironmentVariable('PCT_UPDATER_PUBLIC_KEY', $origPublicKey, 'Process')
  [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $origPrivateKey, 'Process')
  [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', $origPrivateKeyPath, 'Process')
  [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', $origPassword, 'Process')
  [Environment]::SetEnvironmentVariable('ANTHRO_BRIDGE_CHANNEL', $origChannel, 'Process')
  [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $origCargoTarget, 'Process')
  $env:PCT_UPDATER_PUBLIC_KEY = $origPublicKey
  $env:TAURI_SIGNING_PRIVATE_KEY = $origPrivateKey
  $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $origPrivateKeyPath
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $origPassword
  $env:ANTHRO_BRIDGE_CHANNEL = $origChannel
  $env:CARGO_TARGET_DIR = $origCargoTarget
}

Write-Host "`n=========================================="
Write-Host "Test Results: $testsPassed passed, $testsFailed failed" -ForegroundColor $(if ($testsFailed -eq 0) { 'Green' } else { 'Red' })
Write-Host "=========================================="

if ($testsFailed -gt 0) {
  exit 1
}
