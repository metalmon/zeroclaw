#!/usr/bin/env pwsh
# Regenerates the local `main` build/integration branch from upstream + our patches.
#
# Branch model (downstream fork):
#   master              -> pristine mirror of <upstream>/master (fast-forward only; never commit here)
#   PR / local branches -> single-concern patches (see $Branches below)
#   main                -> master + cherry-pick($Branches); THIS is what we build/deploy
#
# When a PR lands upstream, remove its branch from $Branches — its content then
# arrives through master and re-cherry-picking it would only cause a conflict.
#
# Usage (from the repo root):
#   pwsh dev-local/rebuild-main.ps1            # rebuild main locally
#   pwsh dev-local/rebuild-main.ps1 -Push      # rebuild and force-push to the fork

[CmdletBinding()]
param(
    [string]$Remote   = "fork",     # fork remote: source of our branches + push target for main
    [string]$Upstream = "origin",   # upstream we mirror into master
    [switch]$Push                   # also force-push the rebuilt main to $Remote
)

$ErrorActionPreference = "Stop"

# Branches composing main, in cherry-pick order. PR branches first, local-only last.
# Each entry is cherry-picked as the range master..<branch>, so multi-commit
# branches are supported. Drop a line once its PR is merged upstream.
#
# Stacked branches: if a branch is built on top of another branch in this list,
# the script detects this and cherry-picks only the delta (commits from parent
# to current branch) to avoid re-applying already-cherry-picked commits.
$Branches = @(
    "feat/telegram-multi-message",            # PR #8561 - Telegram multi_message streaming mode
    "fix/pipeline-tool-gating",               # PR pending (#9062) - execute_pipeline per-agent tool gating
    "fix/openrouter-stream-provider-extra",   # fix: merge provider_extra in OpenRouter stream_chat (ACP/Thunderbolt)
    "pr2/mcp-embedded-resource-blob-intake",  # PR #9196 - MCP resource.blob materialization + per-call item/byte caps (supersedes fix/mcp-resource-blob-intake #9195, now merged)
    "feat/mcp-image-multimodal",              # stacked on pr2 - materialize MCP type:image/audio into [IMAGE:]/[AUDIO:] markers for the multimodal pipeline
    "fix/mcp-image-role-user",                # relocate MCP tool-result images to a user message (role:tool 400 fix); no upstream PR yet
    "feat/acp-wire-skills",                   # ACP client-delivered skills via _meta extension
    "fix/acp-session-cwd-fallback",           # PR #9536 - ACP session cwd falls back to per-agent workspace (MCP uploads fix)
    "fix/multimodal-token-estimation",        # fix: multimodal token estimation for vision models
    "fix/git-subcommand-classifier",          # #9627 - resolve git subcommand past global options (approval-gate fix)
    "fix/windows-nul-redirect",               # #9633 - accept Windows null device (nul/NUL) as safe redirect target
    "local/git-read-only",                     # local-only: git_read_only risk-profile flag (hard read-only git); stacked on fix/git-subcommand-classifier
    "fix/per-agent-memory-autosave-clean",     # local-only (Bug 3, no PR yet): webhook + heartbeat autosave route to the addressed/heartbeat agent's own memory backend, not the shared default
    "local/dev-tooling"                       # local-only: fork CI (fork-build.yml) + this script; self-restoring, keep last
)

Write-Host "==> fetching $Upstream and $Remote" -ForegroundColor Cyan
git fetch $Upstream --prune
git fetch $Remote --prune

Write-Host "==> fast-forwarding master to $Upstream/master" -ForegroundColor Cyan
git checkout master
git merge --ff-only "$Upstream/master"

Write-Host "==> resetting main to master" -ForegroundColor Cyan
git checkout -B main master

foreach ($b in $Branches) {
    # Check if this branch is stacked on another branch in the list
    $base = "master"
    foreach ($prev in $Branches) {
        if ($prev -eq $b) { break }
        # Check if $prev is an ancestor of $b
        $mergeBase = git merge-base $prev $b 2>$null
        $prevHead = git rev-parse $prev 2>$null
        if ($mergeBase -eq $prevHead) {
            $base = $prev
            break
        }
    }

    Write-Host "==> cherry-pick $base..$b" -ForegroundColor Cyan
    git cherry-pick "$base..$b"
    if ($LASTEXITCODE -ne 0) {
        # Empty when the patch is already on master (merged upstream) — skip and continue.
        $inProgress = Test-Path (Join-Path (git rev-parse --git-dir) "CHERRY_PICK_HEAD")
        $empty = (git status 2>&1 | Out-String) -match "The previous cherry-pick is now empty"
        if ($inProgress -and $empty) {
            Write-Host "    (empty — already on master; skipping $b)" -ForegroundColor Yellow
            git cherry-pick --skip
            if ($LASTEXITCODE -ne 0) { exit 1 }
            continue
        }
        Write-Host "!!! cherry-pick conflict on $b." -ForegroundColor Red
        Write-Host "    Resolve the conflict, then run: git cherry-pick --continue" -ForegroundColor Red
        Write-Host "    (or 'git cherry-pick --abort' to back out), then re-run this script." -ForegroundColor Red
        exit 1
    }
}

Write-Host "==> main rebuilt:" -ForegroundColor Green
git --no-pager log --oneline -n ($Branches.Count + 1)

if ($Push) {
    Write-Host "==> force-pushing main to $Remote" -ForegroundColor Cyan
    git push --force-with-lease $Remote main
}

Write-Host ""
Write-Host "Done. Build with:" -ForegroundColor Green
Write-Host "  cargo build --release --no-default-features --features agent-runtime,gateway,embedded-web,acp-bridge,channel-telegram,plugins-wasm"
