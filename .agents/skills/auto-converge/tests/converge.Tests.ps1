Describe "Auto-Converge Controller & Rust Authoritative Guard Contract (test_controller_guard_contract_agreement)" {
    It "Has required skill artifacts and valid review.schema.json" {
        $SkillRoot = Split-Path -Parent $PSScriptRoot
        $SchemaPath = Join-Path $SkillRoot "schemas\review.schema.json"
        $WorkflowPath = Join-Path $SkillRoot "references\workflow.md"
        $SkillMdPath = Join-Path $SkillRoot "SKILL.md"
        $ScriptPath = Join-Path $SkillRoot "scripts\converge.ps1"

        (Test-Path -LiteralPath $SchemaPath) | Should Be $true
        (Test-Path -LiteralPath $WorkflowPath) | Should Be $true
        (Test-Path -LiteralPath $SkillMdPath) | Should Be $true
        (Test-Path -LiteralPath $ScriptPath) | Should Be $true

        $SchemaRaw = Get-Content -LiteralPath $SchemaPath -Raw -Encoding UTF8
        $SchemaObj = $SchemaRaw | ConvertFrom-Json
        ($SchemaObj.required -contains "verdict") | Should Be $true
        ($SchemaObj.required -contains "approval_binding") | Should Be $true
        ($SchemaObj.properties.findings.items.properties.finding_key.properties.failure_code.enum.Count) | Should Be 14
    }

    It "Fails closed (exit code 1) when binary path is missing or invalid" {
        $SkillRoot = Split-Path -Parent $PSScriptRoot
        $ScriptPath = Join-Path $SkillRoot "scripts\converge.ps1"
        $Result = & pwsh -NoProfile -File $ScriptPath `
            -Subcommand "verify-verdict" `
            -PayloadFile "nonexistent.json" `
            -BinaryPath "C:\nonexistent\anthro-bridge.exe" 2>&1
        $LASTEXITCODE | Should Be 1
    }

    It "Delegates to authoritative binary with canonical auto-converge-guard arguments" {
        $SkillRoot = Split-Path -Parent $PSScriptRoot
        $ScriptPath = Join-Path $SkillRoot "scripts\converge.ps1"
        $TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("poverello-converge-test-" + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
        try {
            $FakeBin = Join-Path $TempDir "fake-guard.cmd"
            Set-Content -LiteralPath $FakeBin -Value "@echo off`r`nif ""%1""==""auto-converge-guard"" exit /b 0`r`nexit /b 2" -Encoding ASCII

            $Out = & pwsh -NoProfile -File $ScriptPath `
                -Subcommand "verify-verdict" `
                -StateFile (Join-Path $TempDir "run_state.json") `
                -PayloadFile (Join-Path $TempDir "verdict.json") `
                -BinaryPath $FakeBin 2>&1

            $LASTEXITCODE | Should Be 0
        } finally {
            Remove-Item -LiteralPath $TempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}
