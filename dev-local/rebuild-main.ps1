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
    [switch]$Push,                  # also force-push the rebuilt main to $Remote
    [string]$StartAt = "",          # resume: keep the current main, skip $Branches before this one (after a hand-resolved conflict)
    [switch]$SkipPicks              # resume: every branch is already on the current main; only run the post-assembly fixups (+ push)
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
    # Dropped 2026-10-04: fix/multimodal-token-estimation superseded upstream (history.rs IMAGE_TOKEN_ESTIMATE / ImageMarkerDisposition price image markers per image).
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
    "fork/authz-profiles-rest",                # /api/authz/profiles CRUD + /api/authz/principals/{id}/profiles bind/unbind (panel Roles page), persisted via config write boundary
    "fork/authz-external-seen",               # GET/DELETE /api/authz/external: external (SSO) subjects recorded on admitted OIDC login (authz-external.json), panel Users page read-only group
    "fork/acp-oidc-dispatch-hardening",        # /acp bearer dispatch by shape (JWT -> oidc.<alias>, zc_ -> pairing, no fallback), wire error data, revocation/401/persist hardening, fmt/clippy fixups
    "fork/oidc-private-ca",                    # [oidc.<alias>].tls_ca_cert_path: trust an on-prem CA for the issuer (discovery/JWKS/introspection), fail closed on a bad file
    "fork/drift-auto-approve",                 # reload drift check normalizes auto_approve like the loader (no permanent "differs from disk" banner)
    "fork/quickstart-channel-types",           # Quickstart "create new channel" picker offers only channel kinds compiled into this binary (+ macOS-only iMessage) and the optional [gateway].onboarding_channel_types allowlist
    "fork/doctor-i18n-ru",                     # ru locale: cli/tools/sections.ftl compiled in + registration; doctor findings through Fluent (cli-doctor-*); RU for doctor/daemon banner/pairing/config warnings/RPC auth, full tool catalog, onboarding pickers (/api/tools + sections localized); voltd hints in the new keys
    "fork/voltd-rebrand",                      # cli.ftl + Rust literals zeroclaw->voltd / ZeroClaw->Volt (--version, banners, hints), panel display_name + logo, [[bin]] voltd (crate names stay zeroclaw)
    "local/dev-tooling"                       # local-only: fork CI (fork-build.yml) + this script; self-restoring, keep last
)

if ($SkipPicks) {
    if ((git branch --show-current) -ne "main") { Write-Host "!!! -SkipPicks needs main checked out" -ForegroundColor Red; exit 1 }
    Write-Host "==> post-assembly fixups only, on the current main" -ForegroundColor Cyan
} elseif ($StartAt) {
    if ($Branches -notcontains $StartAt) { Write-Host "!!! -StartAt '$StartAt' is not in `$Branches" -ForegroundColor Red; exit 1 }
    if ((git branch --show-current) -ne "main") { Write-Host "!!! -StartAt needs main checked out" -ForegroundColor Red; exit 1 }
    Write-Host "==> resuming assembly on the current main at $StartAt" -ForegroundColor Cyan
} else {
    Write-Host "==> fetching $Upstream and $Remote" -ForegroundColor Cyan
    git fetch $Upstream --prune
    git fetch $Remote --prune

    Write-Host "==> fast-forwarding master to $Upstream/master" -ForegroundColor Cyan
    git checkout master
    git merge --ff-only "$Upstream/master"

    Write-Host "==> resetting main to master" -ForegroundColor Cyan
    git checkout -B main master
}

$skipping = [bool]$StartAt
foreach ($b in $Branches) {
    if ($SkipPicks) { break }
    if ($skipping) {
        if ($b -eq $StartAt) { $skipping = $false } else { continue }
    }
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
    # A multi-commit range can stop more than once; drive the sequence until
    # it finishes or hits a conflict we cannot settle here. Do not trust the
    # exit code alone: a pick that rerere staged has been seen to leave the
    # sequencer parked with exit 0, and the next range then fails with
    # "cherry-pick is already in progress" while the loop walks on.
    $gitDir = git rev-parse --git-dir
    while ($LASTEXITCODE -ne 0 -or (Test-Path (Join-Path $gitDir "CHERRY_PICK_HEAD")) -or (Test-Path (Join-Path $gitDir "sequencer"))) {
        $inProgress = Test-Path (Join-Path (git rev-parse --git-dir) "CHERRY_PICK_HEAD")
        if (-not $inProgress) {
            Write-Host "!!! cherry-pick failed on $b with no pick in progress." -ForegroundColor Red
            exit 1
        }
        # Empty when the patch is already on master (merged upstream) — skip it.
        if ((git status 2>&1 | Out-String) -match "The previous cherry-pick is now empty") {
            Write-Host "    (empty — already on master; skipping)" -ForegroundColor Yellow
            git cherry-pick --skip
            continue
        }
        $unmerged = (git diff --name-only --diff-filter=U | Out-String).Trim()
        # rerere's replay can be interrupted by a transient Windows file lock
        # ("could not open <file>: Invalid argument"); re-run it a few times
        # before concluding the conflict needs a human.
        $attempt = 0
        while ($unmerged -and $attempt -lt 5) {
            Start-Sleep -Milliseconds (500 * ($attempt + 1))
            git rerere 2>&1 | Out-Null
            $unmerged = (git diff --name-only --diff-filter=U | Out-String).Trim()
            $attempt++
        }
        if ($unmerged) {
            Write-Host "!!! cherry-pick conflict on $b." -ForegroundColor Red
            Write-Host "    Resolve the conflict, then run: git cherry-pick --continue" -ForegroundColor Red
            Write-Host "    (or 'git cherry-pick --abort' to back out), then re-run this script." -ForegroundColor Red
            exit 1
        }
        # Nothing unmerged: rerere (rerere.enabled + autoUpdate) replayed a
        # recorded resolution and staged it, or a transient Windows lock on
        # .git/logs/HEAD ("unable to append … Permission denied") interrupted
        # the sequence after the tree was ready. Either way: continue, retrying
        # the lock a few times.
        Write-Host "    (tree resolved; continuing $b)" -ForegroundColor Yellow
        $attempt = 0
        do {
            if ($attempt -gt 0) { Start-Sleep -Milliseconds (700 * $attempt) }
            $out = git -c core.editor=true cherry-pick --continue 2>&1 | Out-String
            $code = $LASTEXITCODE
            $attempt++
        } while ($code -ne 0 -and $out -match "Permission denied" -and $attempt -lt 6)
        $LASTEXITCODE = $code
        if ($code -ne 0 -and $out -notmatch "Permission denied") {
            # Not the lock: fall through and let the loop inspect the new state
            # (another conflict further down the range, or an empty pick).
            if (-not (Test-Path (Join-Path (git rev-parse --git-dir) "CHERRY_PICK_HEAD"))) {
                Write-Host $out
                Write-Host "!!! cherry-pick --continue failed on $b." -ForegroundColor Red
                exit 1
            }
        } elseif ($code -ne 0) {
            Write-Host $out
            Write-Host "!!! cherry-pick --continue kept failing on the reflog lock for $b." -ForegroundColor Red
            exit 1
        }
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
    # Upstream's 9-parameter constructor gains one parameter from EACH of
    # feat/acp-wire-skills (wire_skills), feat/mcp-tasks-host (task_supervisor)
    # and fix/mcp-scope-connection-pool (mcp_registry): 12/10 only in the
    # assembled tree, so no single branch can carry the allow.
    @{
        File = "crates/zeroclaw-runtime/src/agent/agent.rs"
        From = "    pub async fn from_pinned_live_config_with_session_cwd_and_mcp_backchannel("
        To   = "    #[allow(clippy::too_many_arguments)] pub async fn from_pinned_live_config_with_session_cwd_and_mcp_backchannel("
    }
    # Cross-branch test-literal drift (fork/* authz stack vs the mcp-tasks chain):
    # fix/per-agent-memory-autosave-clean's webhook-autosave AppState literal has
    # no `token_bindings` (a fork/authz-pairing-principal field), and the two
    # fork `/acp` front-door tests call run_gateway without the chain's
    # task_supervisor/mcp_pool arguments. Neither side can carry the other's
    # field without breaking its own isolated CI. Multi-line From/To use "`n"
    # (the loop below retries with CRLF). Each From occurs exactly once (the
    # run_gateway block twice, deliberately) in the assembled tree.
    @{
        File = "crates/zeroclaw-gateway/src/lib.rs"
        From = "            auto_save: true,`n            task_supervisor: None,`n            mcp_pool: None,`n"
        To   = "            auto_save: true,`n            task_supervisor: None,`n            mcp_pool: None,`n            token_bindings: Arc::new(zeroclaw_config::authz::TokenBindingStore::new_ephemeral()),`n            external_subjects: Arc::new(zeroclaw_gateway::api_authz_external::ExternalSubjectStore::new_ephemeral()),`n"
    }
    @{
        File = "crates/zeroclaw-gateway/src/acp.rs"
        From = "            Some(reload_controls),`n" + ("            None,`n" * 7) + "        ));"
        To   = "            Some(reload_controls),`n" + ("            None,`n" * 9) + "        ));"
    }
    # The upstream route-pinning test calls from_pinned_live_config_… whose
    # parameter list three branches each extend by one (wire_skills, then
    # task_supervisor, then mcp_registry). Their independent edits of the same
    # argument list merge into 11 `None`s where the definition wants
    # `…, &[], None, None`: rewrite the tail once the tree is assembled.
    @{
        File = "crates/zeroclaw-runtime/src/agent/agent.rs"
        From = "            None,`n            None,`n        )`n        .await`n        .expect(`"direct Agent construction`");"
        To   = "            &[],`n            None,`n            None,`n        )`n        .await`n        .expect(`"direct Agent construction`");"
    }
    @{
        File = "crates/zeroclaw-runtime/src/agent/agent.rs"
        From = "            None,`n            None,`n        )`n        .await`n        .expect(`"replacement direct Agent construction`");"
        To   = "            &[],`n            None,`n            None,`n        )`n        .await`n        .expect(`"replacement direct Agent construction`");"
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
    # Multi-line From/To are written with "`n"; a CRLF working tree
    # (core.autocrlf=true on Windows) needs the CRLF spelling instead.
    if ($new -eq $orig -and $fx.From.Contains("`n")) {
        $new = $orig.Replace($fx.From.Replace("`n", "`r`n"), $fx.To.Replace("`n", "`r`n"))
    }
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
