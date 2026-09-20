//! V0.8.0 env-var override mechanism.

use crate::schema::Config;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Accepted env-var prefixes, in precedence order: when the same dotted path
/// is named by more than one prefix (e.g. both `VOLTD_gateway__x` and
/// `ZEROCLAW_gateway__x` are set), the prefix appearing EARLIER in this list
/// wins. `VOLTD_` is the rebrand-forward name; `ZEROCLAW_` is kept for
/// backward compatibility with existing deployments.
const PREFIXES: &[&str] = &["VOLTD_", "ZEROCLAW_"];
const SEP: &str = "__";

/// `[todotracker]` was a daemon schema section in v0.8.3 only; TodoWrite
/// display config now lives in ZeroCode's `zerocode-config.toml`. Removing the
/// schema section would make these previously valid env vars unresolved, and
/// an unresolved path is a hard error — so a working deployment would stop
/// starting after an upgrade. Recognized legacy fields are therefore accepted
/// and ignored (with a migration warning) instead of failing startup.
///
/// This is an exact allowlist: an unknown `ZEROCLAW_todotracker__*` field is
/// still a hard error, so genuine typos keep surfacing.
const LEGACY_TODOTRACKER_FIELDS: [&str; 5] = [
    "enabled",
    "enabled_at_start",
    "location",
    "width",
    "max_height",
];

/// True when `tail` names a recognized legacy `[todotracker]` field that must
/// no longer block daemon startup.
fn is_recognized_legacy_todotracker(tail: &str) -> bool {
    tail.strip_prefix("todotracker")
        .and_then(|rest| rest.strip_prefix(SEP))
        .is_some_and(|field| LEGACY_TODOTRACKER_FIELDS.contains(&field))
}

static NON_OVERRIDABLE_PATHS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["schema_version"]));

#[derive(Debug, Default, Clone)]
pub struct AppliedOverrides {
    pub paths: HashSet<String>,
    pub snapshots: HashMap<String, String>,
}

/// Apply every `VOLTD_<lowercase>` / `ZEROCLAW_<lowercase>` env var to
/// `config`. Returns the set of dotted prop-paths that were overridden plus
/// the pre-override raw values for each. Hard-errors on any env var that
/// doesn't resolve to a known schema path or whose alias fails validation.
///
/// When both prefixes name the same resolved path (e.g. both
/// `VOLTD_gateway__request_timeout_secs` and
/// `ZEROCLAW_gateway__request_timeout_secs` are set), the `VOLTD_` value
/// wins — see [`PREFIXES`] for the precedence order.
pub fn apply_env_overrides(config: &mut Config) -> Result<AppliedOverrides> {
    let mut entries: Vec<(String, String, String, usize)> = std::env::vars()
        .filter_map(|(k, v)| {
            let (rank, tail) = PREFIXES
                .iter()
                .enumerate()
                .find_map(|(rank, prefix)| k.strip_prefix(prefix).map(|tail| (rank, tail)))?;
            (!tail.is_empty()
                && tail
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .then(|| (k.clone(), v, tail.to_string(), rank))
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Pass 1: resolve + validate EVERY matched env var, regardless of which
    // prefix named it and regardless of whether it will end up being the
    // winner for its path. This keeps the hard-fail-on-unknown-path and
    // not-overridable invariants identical for `VOLTD_` and `ZEROCLAW_`
    // alike: a typo'd path is an error no matter which prefix wrote it.
    // `resolve_path`'s map-key creation is idempotent (a second call for an
    // alias that already exists is a no-op, not an error — see
    // `create_map_key`), so resolving the same alias/path twice here (once
    // per colliding prefix) is side-effect-free: no *value* is written in
    // this pass, only map-key scaffolding.
    let mut resolved: Vec<(String, String, String, usize)> = Vec::with_capacity(entries.len());
    for (env_name, value, tail, rank) in entries {
        // Recognized legacy `[todotracker]` vars are accepted and ignored so an
        // upgrade cannot turn a previously working deployment into a daemon
        // that refuses to start. The value has no effect: TodoWrite display is
        // now owned by ZeroCode's `zerocode-config.toml`.
        if is_recognized_legacy_todotracker(&tail) {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"env_var": env_name})),
                "ignoring removed [todotracker] env override; move this setting into zerocode-config.toml"
            );
            continue;
        }
        let path = resolve_path(&tail, config)
            .with_context(|| format!("{env_name} did not resolve to a schema path"))?;
        if NON_OVERRIDABLE_PATHS.contains(path.as_str()) {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Reject)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"env_var": env_name, "path": path})),
                "env override rejected: field is not overridable"
            );
            anyhow::bail!("{env_name} -> {path}: this field is not overridable via env vars");
        }
        resolved.push((env_name, value, path, rank));
    }

    // Pass 2: collapse duplicate resolutions of the same path down to a
    // single winner — lowest `rank` (i.e. `VOLTD_`) beats a higher rank
    // (i.e. `ZEROCLAW_`) — THEN apply each surviving override exactly once.
    // Deduping before any `set_prop`/snapshot call matters: it guarantees
    // the snapshot captures the value that was on `config` before ANY
    // override touched the path (true pre-override state), never an
    // intermediate value a losing duplicate would otherwise have left
    // behind.
    let mut winners: HashMap<String, (String, String, usize)> =
        HashMap::with_capacity(resolved.len());
    for (env_name, value, path, rank) in resolved {
        match winners.get(&path) {
            Some((_, _, existing_rank)) if *existing_rank <= rank => {}
            _ => {
                winners.insert(path, (env_name, value, rank));
            }
        }
    }
    let mut winners: Vec<(String, String, String)> = winners
        .into_iter()
        .map(|(path, (env_name, value, _))| (env_name, value, path))
        .collect();
    winners.sort_by(|a, b| a.0.cmp(&b.0));

    let mut paths: HashSet<String> = HashSet::with_capacity(winners.len());
    let mut snapshots: HashMap<String, String> = HashMap::with_capacity(winners.len());
    for (env_name, value, path) in winners {
        // Snapshot the pre-override raw value via TOML serde walk. Bypasses
        // `Config::get_prop`'s unconditional secret mask: secret fields on
        // `config` carry plaintext (post-`decrypt_secrets`), so the snapshot
        // captures the real value that should be restored at save time.
        let snapshot = raw_value_for_path(config, &path).unwrap_or_default();
        snapshots.insert(path.clone(), snapshot);

        config
            .set_prop(&path, &value)
            .with_context(|| format!("{env_name} → {path}"))?;
        if Config::prop_is_secret(&path) {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"path": path, "env_var": env_name})),
                "Secret applied from env override"
            );
        } else {
            ::zeroclaw_log::record!(
                DEBUG,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_attrs(::serde_json::json!({"path": path, "env_var": env_name})),
                "Env override applied"
            );
        }
        paths.insert(path);
    }
    if !paths.is_empty() {
        ::zeroclaw_log::record!(
            INFO,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_attrs(::serde_json::json!({"count": paths.len()})),
            "Applied env-var config overrides"
        );
    }
    Ok(AppliedOverrides { paths, snapshots })
}

/// Walk an env-var tail against the schema. Map-keyed positions consume one
/// `__`-delimited alias token (which may contain single `_` per the alias
/// validator); everything else resolves via `prop_fields()` lookup.
fn resolve_path(tail: &str, config: &mut Config) -> Result<String> {
    let mut sections = Config::map_key_sections();
    sections.sort_by_key(|s| std::cmp::Reverse(s.path.len()));
    for section in sections {
        let env_pfx: String = section.path.replace('.', SEP);
        let with_sep = format!("{env_pfx}{SEP}");
        let Some(rest) = tail.strip_prefix(&with_sep) else {
            continue;
        };
        let mut parts = rest.splitn(2, SEP);
        let alias = parts.next().filter(|s| !s.is_empty()).ok_or_else(|| {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Reject)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"section": section.path, "tail": tail})),
                "env override path missing alias segment"
            );
            anyhow::Error::msg(format!("missing alias after `{}`", section.path))
        })?;
        let inner = parts.next().unwrap_or("");
        // Propagate the alias-validator's specific error so operators see
        // *why* their alias was rejected (leading underscore, uppercase, …)
        // instead of the generic "Unknown property" that would surface from
        // a downstream `set_prop` against a non-existent map key.
        config.create_map_key(section.path, alias).map_err(|e| {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Reject)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({
                        "section": section.path,
                        "alias": alias,
                        "error": format!("{}", e),
                    })),
                "env override alias rejected by validator"
            );
            anyhow::Error::msg(format!(
                "invalid alias `{alias}` for `{}`: {e}",
                section.path
            ))
        })?;
        let path = if inner.is_empty() {
            format!("{}.{}", section.path, alias)
        } else {
            // Inner segments are `__`-separated snake-case field names — the
            // same casing the prop-path uses, so join them verbatim.
            let inner_path = inner.split(SEP).collect::<Vec<_>>().join(".");
            format!("{}.{}.{}", section.path, alias, inner_path)
        };
        return Ok(path);
    }

    // Non-map path: prop_fields() entries are dotted snake-case field
    // names. Convert to env-form (`.` → `__`) and compare.
    config
        .prop_fields()
        .into_iter()
        .find(|f| f.name.replace('.', SEP) == tail)
        .map(|f| f.name)
        .ok_or_else(|| {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Reject)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"tail": tail})),
                "env override path does not match any schema field"
            );
            anyhow::Error::msg(format!("no schema field has env-form `{tail}`"))
        })
}

pub(crate) fn raw_value_for_path(source: &Config, path: &str) -> Option<String> {
    let table = toml::Value::try_from(source).ok()?;
    let mut current: &toml::Value = &table;
    for segment in path.split('.') {
        let tbl = current.as_table()?;
        current = match tbl.get(segment) {
            Some(v) => v,
            None => tbl.get(&segment.replace('-', "_"))?,
        };
    }
    Some(match current {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

pub fn mask_env_overrides_for_save(
    config_to_save: &mut Config,
    snapshots: &HashMap<String, String>,
) -> Result<()> {
    for (path, value) in snapshots {
        if let Err(err) = config_to_save.set_prop(path, value) {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"path": path, "error": format!("{}", err)})),
                "Save-mask reset failed; field retains default"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn env_test_lock() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    LOCK.lock().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Config;

    struct EnvVarGuard(&'static str);
    impl EnvVarGuard {
        fn set(name: &'static str, value: &str) -> Self {
            // SAFETY: tests serialize on `env_test_lock()`.
            unsafe { std::env::set_var(name, value) };
            Self(name)
        }
    }
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: tests serialize on `env_test_lock()`.
            unsafe { std::env::remove_var(self.0) };
        }
    }

    #[tokio::test]
    async fn walker_resolves_typed_family_alias_default() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set(
            "ZEROCLAW_providers__models__anthropic__default__api_key",
            "sk-ant-fixture",
        );

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");

        assert!(
            applied
                .paths
                .contains("providers.models.anthropic.default.api_key"),
            "kebab-translated path should be recorded: {:?}",
            applied.paths,
        );
        // Secret field round-trips through set_prop into the typed alias.
        assert_eq!(
            config
                .providers
                .models
                .anthropic
                .get("default")
                .and_then(|c| c.base.api_key.as_deref()),
            Some("sk-ant-fixture"),
        );
    }

    #[tokio::test]
    async fn walker_resolves_grok_cli_stdout_limit() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set(
            "ZEROCLAW_providers__models__grok_cli__default__max_acp_stdout_bytes",
            "8388608",
        );

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");

        assert!(
            applied
                .paths
                .contains("providers.models.grok_cli.default.max_acp_stdout_bytes"),
        );
        assert_eq!(
            config
                .providers
                .models
                .grok_cli
                .get("default")
                .and_then(|entry| entry.max_acp_stdout_bytes),
            Some(8_388_608)
        );
    }

    #[tokio::test]
    async fn walker_accepts_alias_with_underscore() {
        let _guard = super::env_test_lock().await;
        let _v1 = EnvVarGuard::set(
            "ZEROCLAW_providers__models__openrouter__prod_v2__api_key",
            "sk-or-fixture",
        );
        let _v2 = EnvVarGuard::set(
            "ZEROCLAW_providers__models__openrouter__prod_v2__model",
            "anthropic/claude-sonnet-4-6",
        );

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");

        assert!(
            applied
                .paths
                .contains("providers.models.openrouter.prod_v2.api_key"),
        );
        assert!(
            applied
                .paths
                .contains("providers.models.openrouter.prod_v2.model"),
        );
        let entry = config
            .providers
            .models
            .openrouter
            .get("prod_v2")
            .expect("alias created");
        assert_eq!(entry.base.api_key.as_deref(), Some("sk-or-fixture"));
        assert_eq!(
            entry.base.model.as_deref(),
            Some("anthropic/claude-sonnet-4-6"),
        );
    }

    #[tokio::test]
    async fn walker_resolves_non_map_gateway_path() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set("ZEROCLAW_gateway__request_timeout_secs", "120");

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");

        assert!(applied.paths.contains("gateway.request_timeout_secs"));
        assert_eq!(config.gateway.request_timeout_secs, 120);
    }

    #[tokio::test]
    async fn walker_rejects_unknown_path() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set("ZEROCLAW_no__such__field", "x");

        let mut config = Config::default();
        let err = apply_env_overrides(&mut config).expect_err("must hard-error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("ZEROCLAW_no__such__field") && msg.contains("did not resolve"),
            "error must name the env var and the failure: {msg}",
        );
    }

    // ── VOLTD_ / ZEROCLAW_ dual-prefix ──────────────────────────────────────

    #[tokio::test]
    async fn walker_resolves_voltd_prefixed_path() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set("VOLTD_gateway__request_timeout_secs", "120");

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");

        assert!(applied.paths.contains("gateway.request_timeout_secs"));
        assert_eq!(config.gateway.request_timeout_secs, 120);
    }

    #[tokio::test]
    async fn voltd_wins_over_zeroclaw_on_collision() {
        let _guard = super::env_test_lock().await;
        let _v1 = EnvVarGuard::set("VOLTD_gateway__request_timeout_secs", "120");
        let _v2 = EnvVarGuard::set("ZEROCLAW_gateway__request_timeout_secs", "999");

        let mut config = Config::default();
        let original_timeout = config.gateway.request_timeout_secs;
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");

        assert!(applied.paths.contains("gateway.request_timeout_secs"));
        assert_eq!(
            config.gateway.request_timeout_secs, 120,
            "VOLTD_ must win over ZEROCLAW_ for the same resolved path",
        );

        // The snapshot must capture the TRUE pre-override value (the
        // default), not the value the losing ZEROCLAW_ duplicate would have
        // left behind had it been applied first.
        let mut to_save = config.clone();
        mask_env_overrides_for_save(&mut to_save, &applied.snapshots).expect("mask succeeds");
        assert_eq!(
            to_save.gateway.request_timeout_secs, original_timeout,
            "snapshot must restore the true pre-override default, not the losing \
             ZEROCLAW_ duplicate's intermediate value",
        );
        // In-memory config is unaffected — VOLTD_'s value is still live.
        assert_eq!(config.gateway.request_timeout_secs, 120);
    }

    #[tokio::test]
    async fn losing_duplicate_malformed_value_is_never_applied_or_validated() {
        let _guard = super::env_test_lock().await;
        // VOLTD_ wins the path collision, so ZEROCLAW_'s value here is
        // dedup'd out in Pass 2 BEFORE `set_prop` (the value-typed parse)
        // ever sees it. Pin that: a value that would fail to parse into
        // `u64` if it were ever applied ("not-a-number" for
        // `request_timeout_secs: u64`) must NOT cause a hard failure, and
        // must not affect the winning VOLTD_ value.
        //
        // This is the true, documented behavior of this implementation: the
        // *path* of every matching env var (winner or loser) is resolved
        // and validated in Pass 1 (so an unknown/non-overridable *path* on
        // a loser still hard-fails — see `voltd_unknown_path_hard_fails_like_legacy`
        // and `schema_version_override_rejected`), but the *value* of a
        // losing duplicate is never parsed or applied at all, because
        // `set_prop` (where value-typed parsing happens) is only called
        // once per path, for the Pass-2 winner.
        let _v1 = EnvVarGuard::set("VOLTD_gateway__request_timeout_secs", "120");
        let _v2 = EnvVarGuard::set("ZEROCLAW_gateway__request_timeout_secs", "not-a-number");

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config).expect(
            "must NOT hard-fail: the losing ZEROCLAW_ duplicate's malformed value is \
             dedup'd out before set_prop would ever parse/validate it",
        );

        assert!(applied.paths.contains("gateway.request_timeout_secs"));
        assert_eq!(
            config.gateway.request_timeout_secs, 120,
            "VOLTD_ wins; the malformed ZEROCLAW_ value must never reach set_prop",
        );
    }

    #[tokio::test]
    async fn voltd_wins_regardless_of_env_var_set_order() {
        let _guard = super::env_test_lock().await;

        // Order 1: ZEROCLAW_ guard created first, then VOLTD_.
        {
            let _v_zc = EnvVarGuard::set("ZEROCLAW_gateway__request_timeout_secs", "999");
            let _v_vd = EnvVarGuard::set("VOLTD_gateway__request_timeout_secs", "120");

            let mut config = Config::default();
            let applied = apply_env_overrides(&mut config).expect("apply succeeds");
            assert!(applied.paths.contains("gateway.request_timeout_secs"));
            assert_eq!(
                config.gateway.request_timeout_secs, 120,
                "VOLTD_ must win when ZEROCLAW_'s guard was created first \
                 (precedence is rank-based, not insertion-order-based)",
            );
        }

        // Order 2: VOLTD_ guard created first, then ZEROCLAW_ — the reverse.
        {
            let _v_vd = EnvVarGuard::set("VOLTD_gateway__request_timeout_secs", "120");
            let _v_zc = EnvVarGuard::set("ZEROCLAW_gateway__request_timeout_secs", "999");

            let mut config = Config::default();
            let applied = apply_env_overrides(&mut config).expect("apply succeeds");
            assert!(applied.paths.contains("gateway.request_timeout_secs"));
            assert_eq!(
                config.gateway.request_timeout_secs, 120,
                "VOLTD_ must win when VOLTD_'s guard was created first too — \
                 same outcome both ways proves rank-based precedence, not \
                 HashMap iteration order or accidental insertion order",
            );
        }
    }

    #[tokio::test]
    async fn voltd_unknown_path_hard_fails_like_legacy() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set("VOLTD_no__such__field", "x");

        let mut config = Config::default();
        let err = apply_env_overrides(&mut config).expect_err("must hard-error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("VOLTD_no__such__field") && msg.contains("did not resolve"),
            "error must name the actual env var and the failure: {msg}",
        );
    }

    #[tokio::test]
    async fn walker_propagates_alias_validator_error() {
        let _guard = super::env_test_lock().await;
        // `_invalid` starts with `_`, which the alias validator rejects.
        // The walker's tail filter accepts `[a-z0-9_]+` so this gets past
        // the prefilter, and the failure must surface as the validator's
        // specific message — not a generic "Unknown property".
        let _v = EnvVarGuard::set(
            "ZEROCLAW_providers__models__anthropic___invalid__api_key",
            "x",
        );

        let mut config = Config::default();
        let err = apply_env_overrides(&mut config).expect_err("must hard-error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("invalid alias") && msg.contains("_invalid"),
            "error must surface the alias validator's message: {msg}",
        );
    }

    #[tokio::test]
    async fn mask_restores_pre_override_snapshot_for_non_secret() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set("ZEROCLAW_gateway__request_timeout_secs", "999");

        let mut config = Config::default();
        let original_timeout = config.gateway.request_timeout_secs;
        let applied = apply_env_overrides(&mut config).expect("apply succeeds");
        assert_eq!(config.gateway.request_timeout_secs, 999);

        let mut to_save = config.clone();
        mask_env_overrides_for_save(&mut to_save, &applied.snapshots).expect("mask succeeds");
        assert_eq!(
            to_save.gateway.request_timeout_secs, original_timeout,
            "non-secret path resets to pre-override snapshot",
        );
        // In-memory config is unchanged — env value still effective for the
        // running process.
        assert_eq!(config.gateway.request_timeout_secs, 999);
    }

    #[tokio::test]
    async fn mask_restores_pre_override_plaintext_for_secret() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set(
            "ZEROCLAW_providers__models__anthropic__default__api_key",
            "sk-ant-from-env",
        );

        // Pre-existing alias with a real plaintext credential (the state
        // after `Config::load_or_init` calls `decrypt_secrets`).
        let mut config = Config::default();
        config
            .providers
            .models
            .ensure("anthropic", "default")
            .expect("typed slot")
            .api_key = Some("sk-ant-on-disk".to_string());

        let applied = apply_env_overrides(&mut config).expect("apply succeeds");
        assert!(
            applied
                .paths
                .contains("providers.models.anthropic.default.api_key"),
        );
        // Env value is live in memory.
        assert_eq!(
            config
                .providers
                .models
                .anthropic
                .get("default")
                .and_then(|c| c.base.api_key.as_deref()),
            Some("sk-ant-from-env"),
        );

        // Save-bound clone restores the pre-override plaintext, NOT the
        // display mask. This is the regression bar for the data-loss bug
        // identified inreview.
        let mut to_save = config.clone();
        mask_env_overrides_for_save(&mut to_save, &applied.snapshots).expect("mask succeeds");
        assert_eq!(
            to_save
                .providers
                .models
                .anthropic
                .get("default")
                .and_then(|c| c.base.api_key.as_deref()),
            Some("sk-ant-on-disk"),
            "secret resets to pre-override plaintext (not the `**** (encrypted)` mask)",
        );
        assert_ne!(
            to_save
                .providers
                .models
                .anthropic
                .get("default")
                .and_then(|c| c.base.api_key.as_deref()),
            Some("**** (encrypted)"),
            "must not corrupt the field with the display mask",
        );
    }

    #[tokio::test]
    async fn schema_version_override_rejected() {
        let _guard = super::env_test_lock().await;
        let _v = EnvVarGuard::set("ZEROCLAW_schema_version", "99");

        let mut config = Config::default();
        let err = apply_env_overrides(&mut config).expect_err("must hard-error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("schema_version") && msg.contains("not overridable"),
            "error must name the path and the reason: {msg}",
        );
    }
    // ── Legacy `[todotracker]` compatibility ────────────────────────────────
    //
    // The section moved out of the daemon schema into ZeroCode's
    // `zerocode-config.toml`. Recognized legacy env vars must not become
    // unresolved paths, because an unresolved path is fatal and would stop a
    // previously working daemon from starting after an upgrade.

    #[tokio::test]
    async fn recognized_legacy_todotracker_env_does_not_block_startup() {
        let _guard = super::env_test_lock().await;
        let _a = EnvVarGuard::set("ZEROCLAW_todotracker__enabled", "false");
        let _b = EnvVarGuard::set("ZEROCLAW_todotracker__enabled_at_start", "true");
        let _c = EnvVarGuard::set("ZEROCLAW_todotracker__location", "left");
        let _d = EnvVarGuard::set("ZEROCLAW_todotracker__width", "40");
        let _e = EnvVarGuard::set("ZEROCLAW_todotracker__max_height", "8");

        let mut config = Config::default();
        let applied = apply_env_overrides(&mut config)
            .expect("recognized legacy todotracker vars must not fail daemon startup");

        // Accepted-and-ignored: no schema path is touched by these vars.
        assert!(
            !applied.paths.iter().any(|p| p.starts_with("todotracker")),
            "legacy vars must not resolve to a schema path: {:?}",
            applied.paths,
        );
    }

    #[tokio::test]
    async fn unknown_legacy_todotracker_field_still_errors() {
        let _guard = super::env_test_lock().await;
        // Not on the exact allowlist: a typo must still surface loudly rather
        // than being silently swallowed by the compatibility shim.
        let _v = EnvVarGuard::set("ZEROCLAW_todotracker__wdith", "40");

        let mut config = Config::default();
        let err = apply_env_overrides(&mut config).expect_err("unknown field must hard-error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("ZEROCLAW_todotracker__wdith") && msg.contains("did not resolve"),
            "error must name the env var and the failure: {msg}",
        );
    }

    #[tokio::test]
    async fn legacy_shim_does_not_shadow_other_sections() {
        let _guard = super::env_test_lock().await;
        // A section that merely *starts with* the legacy name must not be
        // captured by the allowlist prefix check.
        let _v = EnvVarGuard::set("ZEROCLAW_todotracker_extra__enabled", "false");

        let mut config = Config::default();
        let err = apply_env_overrides(&mut config).expect_err("must hard-error");
        assert!(format!("{err:#}").contains("did not resolve"));
    }
}
