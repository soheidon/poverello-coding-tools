# Verification script for Multilingual Documentation Parity in Poverello Coding Tools
param(
    [string]$RepoRoot = ""
)

if (-not $RepoRoot) {
    if ($PSScriptRoot) {
        $RepoRoot = Split-Path -Parent $PSScriptRoot
    } else {
        $RepoRoot = (Get-Location).Path
    }
}

$ErrorActionPreference = "Stop"
$failed = $false

function Report-Pass($msg) {
    Write-Host "[PASS] $msg" -ForegroundColor Green
}

function Report-Fail($msg) {
    Write-Host "[FAIL] $msg" -ForegroundColor Red
    $script:failed = $true
}

Write-Host "=== Step 1: Target Files Existence & Non-Emptiness ===" -ForegroundColor Cyan

$docFamilies = @(
    @{
        Canonical = "README.md"
        Translations = @(
            "docs/README.ja.md",
            "docs/README.de.md",
            "docs/README.es.md",
            "docs/README.fr.md",
            "docs/README.ko.md",
            "docs/README.zh-CN.md",
            "docs/README.zh-TW.md"
        )
    },
    @{
        Canonical = "SPEC.md"
        Translations = @(
            "docs/SPEC.ja.md",
            "docs/SPEC.de.md",
            "docs/SPEC.es.md",
            "docs/SPEC.fr.md",
            "docs/SPEC.ko.md",
            "docs/SPEC.zh-CN.md",
            "docs/SPEC.zh-TW.md"
        )
    },
    @{
        Canonical = "docs/ANTIGRAVITY_MCP.md"
        Translations = @(
            "docs/ANTIGRAVITY_MCP.ja.md",
            "docs/ANTIGRAVITY_MCP.de.md",
            "docs/ANTIGRAVITY_MCP.es.md",
            "docs/ANTIGRAVITY_MCP.fr.md",
            "docs/ANTIGRAVITY_MCP.ko.md",
            "docs/ANTIGRAVITY_MCP.zh-CN.md",
            "docs/ANTIGRAVITY_MCP.zh-TW.md"
        )
    }
)

$standaloneDocs = @("NEWS.md", "docs/DEVELOPMENT.md")
$versionMetadataFiles = @(
    "gui/src-tauri/Cargo.toml",
    "gui/package.json",
    "gui/src-tauri/tauri.conf.json"
)

$allDocPaths = @()
foreach ($f in $docFamilies) {
    $allDocPaths += $f.Canonical
    $allDocPaths += $f.Translations
}
$allDocPaths += $standaloneDocs

Write-Host "Total documentation files to verify: $($allDocPaths.Count) (expected 26)"
Write-Host "Total version metadata files: $($versionMetadataFiles.Count) (expected 3)"
Write-Host "Total targets: $($allDocPaths.Count + $versionMetadataFiles.Count) (expected 29)"

foreach ($relPath in ($allDocPaths + $versionMetadataFiles)) {
    $fullPath = Join-Path $RepoRoot $relPath
    if (-not (Test-Path $fullPath)) {
        Report-Fail "File missing: $relPath"
    } else {
        $len = (Get-Item $fullPath).Length
        if ($len -eq 0) {
            Report-Fail "File is empty: $relPath"
        }
    }
}

if (-not $failed) {
    Report-Pass "All 29 targets exist and are non-empty."
}

Write-Host "`n=== Step 2: Encoding (Strict Valid UTF-8 without BOM) and Line Endings (Strict LF) ===" -ForegroundColor Cyan
$utf8Strict = [System.Text.UTF8Encoding]::new($false, $true)
foreach ($relPath in $allDocPaths) {
    $fullPath = Join-Path $RepoRoot $relPath
    $bytes = [System.IO.File]::ReadAllBytes($fullPath)
    # 1. Check UTF-8 BOM
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        Report-Fail "File has UTF-8 BOM: $relPath"
    }
    # 2. Strict UTF-8 Byte Sequence Validity (throws DecoderFallbackException on invalid sequences)
    try {
        $null = $utf8Strict.GetString($bytes)
    } catch {
        Report-Fail "File contains invalid UTF-8 byte sequences: $relPath ($($_.Exception.Message))"
    }
    # 3. Check for carriage return (CR / 0x0D) to enforce strict LF line endings
    if ($bytes -contains 13) {
        Report-Fail "File contains CRLF line endings (strict LF required): $relPath"
    }
}

Write-Host "`n=== Step 3: Relative Document Links Resolution Across All Documents ===" -ForegroundColor Cyan
foreach ($relDoc in $allDocPaths) {
    $docFullPath = Join-Path $RepoRoot $relDoc
    $content = [System.IO.File]::ReadAllText($docFullPath, [System.Text.Encoding]::UTF8)
    $docDir = Split-Path $docFullPath
    $linkMatches = [regex]::Matches($content, '\[([^\]]+)\]\(([^)]+)\)')
    $docLinkFails = 0
    foreach ($m in $linkMatches) {
        $target = $m.Groups[2].Value
        if ($target -notmatch '^https?://' -and $target -notmatch '^mailto:' -and $target -notmatch '^#') {
            $cleanTarget = ($target -split '#')[0]
            if ($cleanTarget) {
                $targetPath = [System.IO.Path]::GetFullPath((Join-Path $docDir $cleanTarget))
                if (-not (Test-Path $targetPath)) {
                    Report-Fail "$relDoc contains broken relative link: '$target' -> resolved '$targetPath'"
                    $docLinkFails++
                }
            }
        }
    }
    if ($docLinkFails -eq 0) {
        Report-Pass "$relDoc relative links verified ($($linkMatches.Count) total links found)."
    }
}

Write-Host "`n=== Step 4: Multilingual Parity Checks (Structure, Callouts, Tables, Language Bar, Identifiers) ===" -ForegroundColor Cyan

$requiredIdentifiers = @(
    "/pov-mcp-plan",
    "/pov-mcp-revise",
    "/pov-mcp-review",
    "/pov-full-cycle",
    "/pov-plan-only",
    "/pov-build-review",
    "pov-direct.md",
    "codex-cli.md",
    "deepseek-harness.md"
)

$expectedLangDefs = @(
    @{ Name = "English"; Ext = "" },
    @{ Name = "日本語"; Ext = ".ja" },
    @{ Name = "中文(简体)"; Ext = ".zh-CN" },
    @{ Name = "中文(繁體)"; Ext = ".zh-TW" },
    @{ Name = "한국어"; Ext = ".ko" },
    @{ Name = "Français"; Ext = ".fr" },
    @{ Name = "Deutsch"; Ext = ".de" },
    @{ Name = "Español"; Ext = ".es" }
)

function Test-LanguageBar($docRelPath, $lines) {
    if ($lines.Count -eq 0) {
        Report-Fail "$docRelPath is empty, cannot verify language bar"
        return
    }
    $line1 = $lines[0]
    $langLinks = [regex]::Matches($line1, '\[([^\]]+)\]\(([^)]+)\)')
    if ($langLinks.Count -ne 8) {
        Report-Fail "$docRelPath line 1 does not have exactly 8 language switch links (found $($langLinks.Count))"
        return
    }
    $docFullPath = Join-Path $RepoRoot $docRelPath
    $docDir = Split-Path $docFullPath

    for ($i = 0; $i -lt 8; $i++) {
        $expectedDef = $script:expectedLangDefs[$i]
        $label = $langLinks[$i].Groups[1].Value
        $target = $langLinks[$i].Groups[2].Value

        if ($label -ne $expectedDef.Name) {
            Report-Fail "$docRelPath line 1 link #$($i+1) label mismatch: expected '$($expectedDef.Name)', got '$label'"
        }

        $targetFile = [System.IO.Path]::GetFullPath((Join-Path $docDir $target))
        if (-not (Test-Path $targetFile)) {
            Report-Fail "$docRelPath line 1 link '$label' target does not exist: '$target' -> resolved '$targetFile'"
        } else {
            # Verify target file corresponds to the expected language extension
            $targetLeaf = Split-Path $targetFile -Leaf
            if ($expectedDef.Ext -eq "") {
                # English canonical
                if ($targetLeaf -match '\.(ja|zh-CN|zh-TW|ko|fr|de|es)\.md$') {
                    Report-Fail "$docRelPath line 1 link 'English' incorrectly points to localized file: $targetLeaf"
                }
            } else {
                $extPattern = [regex]::Escape($expectedDef.Ext + ".md") + '$'
                if ($targetLeaf -notmatch $extPattern) {
                    Report-Fail "$docRelPath line 1 link '$($expectedDef.Name)' target '$targetLeaf' does not match expected extension '$($expectedDef.Ext).md'"
                }
            }
        }
    }
    Report-Pass "$docRelPath line 1 language switch bar verified (8 languages + destination match)."
}

foreach ($family in $docFamilies) {
    $canonPath = Join-Path $RepoRoot $family.Canonical
    $canonContent = [System.IO.File]::ReadAllText($canonPath, [System.Text.Encoding]::UTF8)
    $canonLines = $canonContent -split "\r?\n"

    # Extract canonical structural elements
    $canonHeadings = $canonLines | Where-Object { $_ -match '^#{1,4}\s' }
    $canonHeadingLevels = $canonHeadings | ForEach-Object { ($_ -replace '^(\#{1,4})\s.*$', '$1').Length }
    $canonCodeBlocks = ($canonLines | Where-Object { $_ -match '^```' }).Count
    $canonCallouts = ($canonLines | Where-Object { $_ -match '^>\s*\[!(TIP|NOTE|IMPORTANT|WARNING|CAUTION)\]' }).Count
    $canonTableSeparators = $canonLines | Where-Object { $_ -match '^\|?\s*[-:]+\s*\|' }

    Write-Host "`nFamily [$($family.Canonical)]: $($canonHeadings.Count) headings, $canonCodeBlocks code blocks, $canonCallouts callouts, $($canonTableSeparators.Count) tables" -ForegroundColor Yellow

    # Verify Canonical Language Bar
    Test-LanguageBar $family.Canonical $canonLines

    foreach ($trans in $family.Translations) {
        $transPath = Join-Path $RepoRoot $trans
        $transContent = [System.IO.File]::ReadAllText($transPath, [System.Text.Encoding]::UTF8)
        $transLines = $transContent -split "\r?\n"

        # 1. Check Language Switch Bar (Line 1)
        Test-LanguageBar $trans $transLines

        # 2. Check Headings count & exact level sequence
        $transHeadings = $transLines | Where-Object { $_ -match '^#{1,4}\s' }
        if ($transHeadings.Count -ne $canonHeadings.Count) {
            Report-Fail "$trans heading count ($($transHeadings.Count)) does not match canonical ($($canonHeadings.Count))"
        } else {
            $transHeadingLevels = $transHeadings | ForEach-Object { ($_ -replace '^(\#{1,4})\s.*$', '$1').Length }
            $levelMismatches = 0
            for ($i = 0; $i -lt $canonHeadingLevels.Count; $i++) {
                if ($canonHeadingLevels[$i] -ne $transHeadingLevels[$i]) {
                    Report-Fail "$trans heading #$($i+1) level mismatch: expected H$($canonHeadingLevels[$i]), got H$($transHeadingLevels[$i])"
                    $levelMismatches++
                }
            }
            if ($levelMismatches -eq 0) {
                Report-Pass "$trans heading level sequence ($($transHeadings.Count) headings) matches canonical."
            }
        }

        # 3. Check Code block count
        $transCodeBlocks = ($transLines | Where-Object { $_ -match '^```' }).Count
        if ($transCodeBlocks -ne $canonCodeBlocks) {
            Report-Fail "$trans code block count ($transCodeBlocks) does not match canonical ($canonCodeBlocks)"
        } else {
            Report-Pass "$trans code block count ($transCodeBlocks) matches canonical."
        }

        # 4. Check Callout count
        $transCallouts = ($transLines | Where-Object { $_ -match '^>\s*\[!(TIP|NOTE|IMPORTANT|WARNING|CAUTION)\]' }).Count
        if ($transCallouts -ne $canonCallouts) {
            Report-Fail "$trans callout count ($transCallouts) does not match canonical ($canonCallouts)"
        } else {
            Report-Pass "$trans callout count ($transCallouts) matches canonical."
        }

        # 5. Check Table column structure (Separators & Column counts)
        $transTableSeparators = $transLines | Where-Object { $_ -match '^\|?\s*[-:]+\s*\|' }
        if ($transTableSeparators.Count -ne $canonTableSeparators.Count) {
            Report-Fail "$trans table count ($($transTableSeparators.Count)) does not match canonical ($($canonTableSeparators.Count))"
        } else {
            $colMismatches = 0
            for ($i = 0; $i -lt $canonTableSeparators.Count; $i++) {
                $canonCols = ($canonTableSeparators[$i].Trim('|') -split '\|').Count
                $transCols = ($transTableSeparators[$i].Trim('|') -split '\|').Count
                if ($canonCols -ne $transCols) {
                    Report-Fail "$trans table #$($i+1) column count mismatch: expected $canonCols, got $transCols"
                    $colMismatches++
                }
            }
            if ($colMismatches -eq 0) {
                Report-Pass "$trans table column structure ($($transTableSeparators.Count) tables) matches canonical."
            }
        }

        # 6. Check Required Untranslated Identifiers
        $missingIds = @()
        foreach ($id in $requiredIdentifiers) {
            if ($canonContent -match [regex]::Escape($id)) {
                if ($transContent -notmatch [regex]::Escape($id)) {
                    $missingIds += $id
                    Report-Fail "$trans missing required untranslated identifier: '$id'"
                }
            }
        }
        if ($missingIds.Count -eq 0) {
            Report-Pass "$trans contains all required untranslated identifiers."
        }
    }
}

if (-not $failed) {
    Write-Host "`nAll multilingual parity checks PASSED!" -ForegroundColor Green
} else {
    Write-Host "`nParity verification finished with errors." -ForegroundColor Red
    exit 1
}
