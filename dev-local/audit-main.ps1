# Post-assembly audit: does the rebuilt `main` actually carry every branch's work?
#
# rebuild-main.ps1 replays the branches onto a fresh master with cherry-pick, and
# every conflict is resolved by hand. A hand resolution that takes one side whole
# keeps the commit but drops its content, and nothing notices: the build still
# compiles and the tests still pass. That is how the panel lost its translations
# twice. This script is the missing notice.
#
# Two checks, both cheap:
#   1. commits   - `git cherry main <branch>` must report nothing missing.
#   2. sole-owner files - a file that exactly one branch touches must be byte
#      identical in main. Files that several branches touch are listed as
#      "shared" and left to the eye: there the difference is legitimate.
#
# Usage: pwsh dev-local/audit-main.ps1 [-Main main] [-Master master]
# Exit code 1 on any missing commit or any sole-owner file that differs.

param(
    [string]$Main = "main",
    [string]$Master = "master"
)

$ErrorActionPreference = "Stop"

$script = Join-Path $PSScriptRoot "rebuild-main.ps1"
if (-not (Test-Path $script)) { throw "rebuild-main.ps1 not found next to this script" }

# The branch list lives in the rebuild script; read it from there so the two can
# never disagree about what main is supposed to contain. Parsed, not sourced:
# running the rebuild script for its variables would start an assembly.
$lines = Get-Content $script
$from = ($lines | Select-String -SimpleMatch '$Branches = @(' | Select-Object -First 1).LineNumber
if (-not $from) { throw "rebuild-main.ps1 has no `$Branches list" }
$Branches = @()
for ($i = $from; $i -lt $lines.Count; $i++) {
    $line = $lines[$i].Trim()
    if ($line -eq ")") { break }
    if ($line -match '^\s*"([^"]+)"') { $Branches += $Matches[1] }
}
if ($Branches.Count -eq 0) { throw "no branches parsed out of rebuild-main.ps1" }

$owners = @{}   # path -> list of branches touching it
$missing = @()

foreach ($b in $Branches) {
    $gone = @(git cherry $Main $b 2>$null | Where-Object { $_ -like "+ *" })
    if ($gone.Count -gt 0) {
        $missing += [pscustomobject]@{ Branch = $b; Commits = $gone.Count }
        Write-Host "!!! $b : $($gone.Count) commit(s) not in $Main" -ForegroundColor Red
        foreach ($c in $gone) {
            $sha = $c.Substring(2)
            Write-Host "      $(git log -1 --format='%h %s' $sha)"
        }
    }
    foreach ($f in (git diff --name-only "$Master...$b" 2>$null)) {
        if (-not $owners.ContainsKey($f)) { $owners[$f] = @() }
        $owners[$f] += $b
    }
}

$drifted = @()
$shared = 0
foreach ($f in $owners.Keys) {
    if ($owners[$f].Count -gt 1) { $shared++; continue }
    $b = $owners[$f][0]
    $inBranch = git rev-parse "${b}:${f}" 2>$null
    $inMain = git rev-parse "${Main}:${f}" 2>$null
    if ($inBranch -ne $inMain) {
        $drifted += [pscustomobject]@{ Branch = $b; File = $f }
    }
}

Write-Host ""
Write-Host "branches: $($Branches.Count)   sole-owner files checked: $($owners.Count - $shared)   shared files (not checked): $shared"

if ($drifted.Count -gt 0) {
    Write-Host ""
    Write-Host "!!! sole-owner files that differ in ${Main}: $($drifted.Count)" -ForegroundColor Red
    foreach ($d in $drifted | Sort-Object Branch, File) {
        Write-Host "      $($d.Branch)  $($d.File)"
        Write-Host "        git diff $($d.Branch) ${Main} -- $($d.File)"
    }
}

if ($missing.Count -eq 0 -and $drifted.Count -eq 0) {
    Write-Host "audit clean: ${Main} carries every branch's commits and every sole-owner file" -ForegroundColor Green
    exit 0
}
exit 1
