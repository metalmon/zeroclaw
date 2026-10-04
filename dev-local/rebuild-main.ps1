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
    # Dropped 2026-09-25: feat/telegram-multi-message (#8561) merged upstream 2026-09-20 as 441df131.
    # Dropped 2026-09-11: fix/heartbeat-composite-channel-target (#10671 / issue #10670) merged upstream.
    # Dropped 2026-10-04: pr-10604-opencode-session (#10604) merged upstream as 4244a30d25.
    # Dropped 2026-08-29: fix/telegram-reply-thread-history (#10418 / issue #10237) merged upstream.
    # fix/pipeline-tool-gating DROPPED: #9062/#7960 closed as duplicate; upstream
    # closed #7947 (execute_pipeline confused-deputy) with its own per-agent
    # ToolAccessPolicy gating, now in master. The local branch is redundant and
    # conflicts with master's implementation.
    # Dropped: fix/openrouter-stream-provider-extra — superseded upstream. Master
    # now has OpenRouterModelProvider::merge_extra_body + a builder .extra_body()
    # setter applied across chat/stream_chat, so the local branch is redundant and
    # its tests use the removed ::new constructor (builder pattern now).
    # Dropped 2026-08-31: pr2/mcp-embedded-resource-blob-intake (#9196) merged upstream.
    # feat/mcp-image-multimodal was stacked on pr2; pr2 is now in master and only its
    # own single commit remains — verified 2026-09-02 to cherry-pick cleanly onto master.
    # Dropped 2026-10-04: feat/mcp-image-multimodal merged upstream as e66c219e6b (all 3 commits); the chain below now starts at fix/per-agent-memory-autosave-clean.
    "fix/mcp-image-role-user",                # PR #10502 - tool_result_image_policy=relocate (now the default) moves role:tool images into a user message (role:tool 400 fix); adds the variant on top of #10448's enum, already in master
    "feat/acp-wire-skills",                   # ACP client-delivered skills via _meta extension
    # Dropped 2026-08-21: fix/acp-session-cwd-fallback (#9536) merged upstream.
    "fix/multimodal-token-estimation",        # fix: multimodal token estimation for vision models
    # Dropped 2026-09-19: fix/git-subcommand-classifier (#9635, closes #9627) merged upstream as 9702c650.
    # Dropped 2026-08-21: fix/windows-nul-redirect (#9636) merged upstream.
    "local/git-read-only",                     # local-only: git_read_only risk-profile flag (hard read-only git); was stacked on fix/git-subcommand-classifier — now applies on master's classifier (#9635 merged), so re-base local/git-read-only onto master if policy.rs conflicts at assembly
    "fix/per-agent-memory-autosave-clean",     # local-only (Bug 3, no PR yet): webhook + heartbeat autosave route to the addressed/heartbeat agent's own memory backend, not the shared default
    "fix/session-ownership-scope",             # local-only (#9646, no PR yet): scope session tools (list/read) to the calling agent's ownership
    "fix/knowledge-per-agent-attribution",     # local-only (#9647, no PR yet): knowledge graph per-agent attribution + scoping, gated behind [knowledge] per_agent_scope (default off)
    "feat/mcp-tasks-host",                     # local-only (no PR yet): MCP tasks-extension host — supervisor polls task-augmented tool calls (kutsu place_call) and injects the result reactively into the originating session
    "fix/mcp-scope-connection-pool",           # local-only (no PR yet): stacked on feat/mcp-tasks-host — daemon-owned per-scope MCP connection pool shared by all sessions + the task poller (one process per scope; fixes kutsu double-spawn)
    "feat/acp-ui-resource-artifacts",          # local-only (no PR): ACP ui:// UI-resource artifacts — canvas render emits a ui:// text resource on tool_call_update (+ content_file input, + store:false session isolation); paired with the Thunderbolt client. Off pristine master, applies clean.
    # ── Enterprise authz re-host on upstream #8289 (2026-10-04). A STACK: each
    # branch is based on the previous one (script cherry-picks only the delta).
    # Order matters; keep contiguous. Source of truth for the design:
    # _local/ledger-f4-8289-migration.md (gitignored) on feat/fork-rebrand-voltd.
    "fork/macros-natural-key",                 # Configurable derive: create_map_key seeds the declared #[natural_key] field (upstream-PR candidate)
    "fork/infra-clippy-nonminimal-bool",       # upstream zeroclaw-infra nonminimal_bool under clippy 1.96.1 -D warnings (drop when upstream fixes it)
    "fork/authz-pairing-principal",            # pairing-by-code -> DISTINCT principal: [[authz.principals]] + TokenBindingStore + PairingAuthProvider merged into upstream PrincipalResolver; frozen -32602/agent_not_permitted gate
    "fork/agent-display-name",                 # [agents.<alias>].display_name + principal-scoped ACP initialize roster {alias,display_name,default}
    "fork/acp-surface-split",                  # [gateway.public]: /acp alone on a TLS public listener, admin/api/dashboard private
    "fork/authz-roles-rest",                   # GET /api/authz/principals, /admin/paircode/new?principal=, CLI get-paircode --principal
    "fork/acp-oidc-dispatch-hardening",        # /acp bearer dispatch by shape (JWT -> oidc.<alias>, zc_ -> pairing, no fallback), wire error data, revocation/401/persist hardening, fmt/clippy fixups
    "fork/voltd-rebrand",                      # cli.ftl zeroclaw->voltd / ZeroClaw->Volt, panel display_name + logo, [[bin]] voltd (crate names stay zeroclaw)
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
    # Check if this branch is stacked on another branch in the list.
    # Keep the LAST (closest) ancestor, not the first: with a multi-level
    # stack (e.g. mcp-image-role-user <- per-agent-memory <- mcp-tasks-host)
    # an earlier branch is also an ancestor, and cherry-picking from it would
    # re-apply the intermediate branch's commits. $Branches is topologically
    # ordered (parents before children), so the last match is the direct base.
    $base = "master"
    foreach ($prev in $Branches) {
        if ($prev -eq $b) { break }
        # Check if $prev is an ancestor of $b
        $mergeBase = git merge-base $prev $b 2>$null
        $prevHead = git rev-parse $prev 2>$null
        if ($mergeBase -eq $prevHead) {
            $base = $prev
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

# --- Post-assembly test-drift fixups (patch final `main`, not cherry-picked) ---
# These re-apply on every rebuild automatically (literal text replace on the
# assembled tree — never a cherry-pick, so never conflicts). Use ONLY for
# test-only compile drift we cannot fix on the owning branch.
#
# (The #8561 telegram fixups were removed 2026-10-04: #8561 merged upstream.)
$fixups = @(
    # TEMPORARY (fork clippy `-D warnings`): lint drift that only appears in the
    # ASSEMBLED main under the fork build's feature set — the dead_code test helpers
    # are live on their own branches, and the unused_mut fires only for this cfg
    # combination — so none can be fixed on a single owning branch. Attributes are
    # inserted on the SAME line as the item; `cargo fmt` (run below) reflows them onto
    # their own line. Each From is verified to occur once in the assembled tree.
    # REMOVE an entry when upstream / the owning branch absorbs its fix.
    @{
        File = "crates/zeroclaw-providers/src/copilot.rs"
        From = "let mut builder = cap_std::fs::DirBuilder::new();"
        To   = "#[allow(unused_mut)] let mut builder = cap_std::fs::DirBuilder::new();"
    }
    @{
        File = "crates/zeroclaw-runtime/src/skills/mod.rs"
        From = "mod copy_tests {"
        To   = "mod copy_tests { #![allow(dead_code, unused_imports)]"
    }
    @{
        File = "crates/zeroclaw-runtime/src/tools/delegate.rs"
        From = "mod tests {"
        To   = "mod tests { #![allow(dead_code, unused_imports)]"
    }
    @{
        File = "crates/zeroclaw-tools/src/mcp_client.rs"
        From = "pub(crate) async fn advertised_tasks(&self) -> bool {"
        To   = "#[allow(dead_code)] pub(crate) async fn advertised_tasks(&self) -> bool {"
    }
    # `anyhow!` is disallowed by clippy.toml; mcp_tasks/mod.rs (feat/mcp-tasks-host)
    # still uses it. Proper home is that branch; patched here to keep main green.
    # Single-quoted because the string contains backticks (`{alias}`).
    @{
        File = 'crates/zeroclaw-runtime/src/mcp_tasks/mod.rs'
        From = 'anyhow::anyhow!("no MCP servers for scope `{alias}`")'
        To   = 'anyhow::Error::msg(format!("no MCP servers for scope `{alias}`"))'
    }
)
$patched = $false
foreach ($fx in $fixups) {
    $path = Resolve-Path -LiteralPath $fx.File
    $orig = [System.IO.File]::ReadAllText($path)
    $new  = $orig.Replace($fx.From, $fx.To)
    if ($new -ne $orig) {
        [System.IO.File]::WriteAllText($path, $new)
        git add -- $fx.File
        $patched = $true
        Write-Host "==> post-assembly fixup applied: $($fx.File)" -ForegroundColor Yellow
    }
}
if ($patched) {
    # The literal replacements can exceed rustfmt's line width; format so the
    # `Format` CI gate stays green. main was fmt-clean pre-replace, so this only
    # touches the wrapped lines.
    Write-Host "==> formatting post-assembly fixups (cargo fmt)" -ForegroundColor Yellow
    cargo fmt
    git add -u
    git commit -q -m "test: post-assembly drift fixups (remove when the owning PRs refresh)"
}

Write-Host "==> main rebuilt:" -ForegroundColor Green
git --no-pager log --oneline -n ($Branches.Count + 1)

if ($Push) {
    Write-Host "==> force-pushing main to $Remote" -ForegroundColor Cyan
    git push --force-with-lease $Remote main
}

Write-Host ""
Write-Host "Done. Build with:" -ForegroundColor Green
Write-Host "  cargo build --release --no-default-features --features agent-runtime,gateway,embedded-web,acp-bridge,channel-telegram"
