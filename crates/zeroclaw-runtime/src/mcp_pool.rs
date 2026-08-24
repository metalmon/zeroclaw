//! Daemon-owned per-scope MCP connection pool.
//!
//! [`McpConnectionPool`] is the shared owner of live MCP server connections,
//! keyed by [`ScopeKey`] (today just an agent alias; a future `(user, alias)`
//! pair can be added without touching call sites). It replaces ad hoc
//! per-caller `McpRegistry::connect_all*` calls with a lazy, reconciling
//! get-or-build: [`McpConnectionPool::registry_for`] reuses a scope's alive,
//! config-unchanged server connections and only reconnects the servers that
//! actually changed or died, so a long-lived stdio child (e.g. a task-capable
//! MCP server) is not torn down and respawned on every call into the scope.
//!
//! This is the daemon-side connection owner; callers (agent turns, the MCP
//! task supervisor, etc.) borrow an `Arc<McpRegistry>` from it rather than
//! owning their own connections.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use zeroclaw_config::schema::{Config, McpServerConfig};

use crate::tools::{McpRegistry, McpServer};

/// Identifies an MCP connection scope. Only `alias` is populated today; kept
/// as a struct (not a bare `String`) so a future `(user, alias)` pair can be
/// added without touching call sites.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ScopeKey {
    alias: String,
}

impl ScopeKey {
    pub fn for_alias(alias: &str) -> Self {
        Self {
            alias: alias.to_string(),
        }
    }
}

/// One scope's live MCP connections: per-server handles keyed by server name
/// (paired with the config each was built with, so a later call can detect
/// staleness via `PartialEq` without re-connecting), plus the registry
/// assembled from them.
struct ScopeEntry {
    handles: HashMap<String, (McpServerConfig, McpServer)>,
    registry: Arc<McpRegistry>,
}

/// Daemon-owned pool of per-scope MCP connections.
///
/// Holds the live `Config` behind the same `Arc<parking_lot::RwLock<Config>>`
/// used across the runtime crate, and a `tokio::sync::Mutex`-guarded map of
/// per-scope connection state. `registry_for` is the sole entry point:
/// get-or-build, lazy, per-server reconciliation — see its doc comment for
/// the exact algorithm and the lock/await discipline it follows.
pub struct McpConnectionPool {
    config: Arc<parking_lot::RwLock<Config>>,
    scopes: Mutex<HashMap<ScopeKey, ScopeEntry>>,
}

impl McpConnectionPool {
    pub fn new(config: Arc<parking_lot::RwLock<Config>>) -> Arc<Self> {
        Arc::new(Self {
            config,
            scopes: Mutex::new(HashMap::new()),
        })
    }

    /// Convenience constructor for call sites that only hold an owned
    /// `Config` snapshot rather than an existing `Arc<parking_lot::RwLock<Config>>`
    /// handle — e.g. the daemon's startup/reload loop in the root `zeroclaw`
    /// binary crate, which does not depend on `parking_lot` directly (it is
    /// dev-dependency-only there; see the root `Cargo.toml` comment "moved
    /// out of `[dependencies]` — no `src/` usage"). Wraps `config` in a
    /// fresh lock, mirroring how `McpTaskSupervisor::start` takes an owned
    /// `Config` and keeps its own independent snapshot rather than sharing
    /// `RpcContext.config`'s handle.
    pub fn from_owned_config(config: Config) -> Arc<Self> {
        Self::new(Arc::new(parking_lot::RwLock::new(config)))
    }

    /// Get-or-build the shared registry for `alias`. Returns `None` when the
    /// scope has no MCP servers granted (secure by default: an alias with no
    /// `mcp_bundles` grant is an empty scope, not an error).
    ///
    /// Algorithm (lazy, per-server reconcile):
    /// 1. Read the scope's configured servers from `Config` and immediately
    ///    drop the `parking_lot::RwLock` read guard — it is never held across
    ///    an `.await` point (parking_lot guards are not designed to survive
    ///    a suspend; holding one across an await risks blocking other sync
    ///    readers/writers for the duration of an MCP handshake, and
    ///    `parking_lot::RwLockReadGuard` is `!Send` in the general case,
    ///    which would fail to compile once actually held across `.await`
    ///    inside a `Send` future). The servers `Vec` is cloned out instead.
    /// 2. Lock `scopes` (a `tokio::sync::Mutex`, safe to hold across
    ///    `.await`) and take the existing entry for this scope, if any.
    /// 3. For each configured server: reuse the existing handle iff its
    ///    stored config still equals the current config (`McpServerConfig:
    ///    PartialEq`) AND it still passes `health_check()`; otherwise
    ///    reconnect via `McpServer::connect_advertising`, skipping (with a
    ///    warn log) on error — mirroring `McpRegistry::connect_all_mixed`'s
    ///    non-fatal-per-server handling.
    /// 4. If every server was reused unchanged AND the reused set's names
    ///    match the existing entry's exactly (no server added/removed),
    ///    return `Arc::clone` of the existing registry without rebuilding —
    ///    no churn, so a live child process is left completely alone.
    /// 5. Otherwise assemble a fresh `McpRegistry::from_servers`, store it as
    ///    the new `ScopeEntry`, and return it. Any replaced/removed
    ///    `McpServer` handle is dropped here; its stdio child is reaped by
    ///    `kill_on_drop` once no other `Arc` clone (e.g. an in-flight task
    ///    poller) still references it.
    ///
    /// Lock/await discipline note: the `scopes` tokio `Mutex` guard IS held
    /// across the `.await` calls to `McpServer::connect_advertising` in step
    /// 3 — a deliberate v1 simplification. This serializes connection
    /// reconciliation across the whole pool (one scope's slow reconnect
    /// blocks another scope's `registry_for` call from proceeding past the
    /// lock), which is acceptable for v1's expected call volume (per-turn,
    /// not per-tool-call) but is a known scalability tradeoff a future
    /// revision could address by connecting outside the lock and only
    /// re-taking it to publish the result.
    pub async fn registry_for(&self, alias: &str) -> Option<Arc<McpRegistry>> {
        let servers: Vec<McpServerConfig> = {
            let cfg = self.config.read();
            cfg.mcp_servers_for_agent(alias)
        };
        if servers.is_empty() {
            return None;
        }

        let key = ScopeKey::for_alias(alias);
        let mut scopes = self.scopes.lock().await;
        let existing = scopes.remove(&key);

        let mut new_handles: HashMap<String, (McpServerConfig, McpServer)> =
            HashMap::with_capacity(servers.len());
        let mut all_reused = true;

        for cfg in &servers {
            let reused = existing
                .as_ref()
                .and_then(|entry| entry.handles.get(&cfg.name))
                .filter(|(stored_cfg, handle)| stored_cfg == cfg && handle.health_check())
                .map(|(_, handle)| handle.clone());

            if let Some(handle) = reused {
                new_handles.insert(cfg.name.clone(), (cfg.clone(), handle));
                continue;
            }

            all_reused = false;
            let advertise = cfg.tasks_enabled_effective();
            match McpServer::connect_advertising(cfg.clone(), advertise).await {
                Ok(handle) => {
                    new_handles.insert(cfg.name.clone(), (cfg.clone(), handle));
                }
                // Non-fatal — log and continue with remaining servers,
                // matching McpRegistry::connect_all_mixed's per-server
                // skip-on-error behavior.
                Err(e) => {
                    ::zeroclaw_log::record!(
                        WARN,
                        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                            .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                            .with_attrs(::serde_json::json!({
                                "mcp_server": &cfg.name,
                                "scope": alias,
                            })),
                        &format!(
                            "mcp_pool: failed to connect MCP server `{}` for scope `{}`: {:#}",
                            cfg.name, alias, e
                        )
                    );
                }
            }
        }

        // Unchanged iff every configured server was reused (`all_reused`)
        // AND the reused name set exactly matches the existing entry's —
        // `all_reused` alone would miss a server that was removed from
        // config (fewer names now than before).
        let names_unchanged = existing
            .as_ref()
            .map(|entry| {
                entry.handles.len() == new_handles.len()
                    && entry.handles.keys().all(|k| new_handles.contains_key(k))
            })
            .unwrap_or(false);

        if all_reused && names_unchanged {
            let entry = existing.expect("names_unchanged implies existing.is_some()");
            let registry = Arc::clone(&entry.registry);
            scopes.insert(key, entry);
            return Some(registry);
        }

        let registry = Arc::new(
            McpRegistry::from_servers(new_handles.values().map(|(_, s)| s.clone()).collect()).await,
        );
        scopes.insert(
            key,
            ScopeEntry {
                handles: new_handles,
                registry: Arc::clone(&registry),
            },
        );
        Some(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::schema::AliasedAgentConfig;
    // `McpBundleConfig`/`McpTransport` are only referenced by the
    // stdio-fake-server test helpers below, which are `#[cfg(unix)]` (real
    // child-process spawning has no Windows equivalent here); gate the
    // import the same way so non-unix builds don't warn on unused imports.
    #[cfg(unix)]
    use zeroclaw_config::schema::{McpBundleConfig, McpTransport};

    #[cfg(unix)]
    fn write_executable_script(path: &std::path::Path, body: &[u8]) {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let mut script = std::fs::File::create(path).expect("create script");
        script.write_all(body).expect("write script");
        drop(script);
        let mut permissions = std::fs::metadata(path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod script");
    }

    /// A minimal MCP stdio server: replies to `initialize` and `tools/list`
    /// with a single tool named after the script's own file stem, then
    /// blocks reading stdin so the process stays alive as a live handle for
    /// the pool to reuse or reap. Newline-delimited JSON-RPC framing and the
    /// `case "$line" in *'"method":"..."'*)` dispatch style mirror the
    /// proven fake-server pattern used by `zeroclaw-tools`'s own
    /// `mcp_client.rs` unit tests (e.g.
    /// `stdio_concurrent_calls_route_mismatched_and_out_of_order_replies`),
    /// reimplemented here because that module's test-only helpers are not
    /// importable cross-crate.
    #[cfg(unix)]
    fn fake_mcp_server_script(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(format!("{name}.sh"));
        let body = format!(
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf '%s\n' "{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":\"{name}\",\"version\":\"0.0.0\"}}}}}}"
      ;;
    *'"method":"tools/list"'*)
      printf '%s\n' "{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"tools\":[{{\"name\":\"{name}_tool\",\"description\":\"d\",\"inputSchema\":{{\"type\":\"object\"}}}}]}}}}"
      ;;
  esac
done
"#,
        );
        write_executable_script(&path, body.as_bytes());
        path
    }

    #[cfg(unix)]
    fn stdio_config(name: &str, script: &std::path::Path) -> McpServerConfig {
        McpServerConfig {
            name: name.to_string(),
            command: script.display().to_string(),
            transport: McpTransport::Stdio,
            tool_timeout_secs: Some(10),
            ..Default::default()
        }
    }

    /// Build a `Config` that grants `alias` exactly the MCP servers in
    /// `servers` (each `(name, script_path)`), via a one-off
    /// `[mcp_bundles.<alias>-bundle]` referenced by `[agents.<alias>]`.
    /// Mirrors `zeroclaw-config`'s own `config_with_mcp_bundles` /
    /// `mcp_servers_for_agent_grants_only_via_agent_bundles` test pattern.
    #[cfg(unix)]
    fn cfg_with_agent(
        alias: &str,
        servers: &[(&str, &std::path::Path)],
    ) -> Arc<parking_lot::RwLock<Config>> {
        let mut config = Config::default();
        for (name, script) in servers {
            config.mcp.servers.push(stdio_config(name, script));
        }
        let bundle_alias = format!("{alias}-bundle");
        config.mcp_bundles.insert(
            bundle_alias.clone(),
            McpBundleConfig {
                servers: servers.iter().map(|(n, _)| n.to_string()).collect(),
                exclude: Vec::new(),
            },
        );
        config.agents.insert(
            alias.to_string(),
            AliasedAgentConfig {
                mcp_bundles: vec![bundle_alias],
                ..AliasedAgentConfig::default()
            },
        );
        Arc::new(parking_lot::RwLock::new(config))
    }

    /// A `Config` that resolves `alias` (via `[agents.<alias>]` with an
    /// empty `mcp_bundles`) but grants it zero MCP servers.
    fn cfg_with_bare_agent(alias: &str) -> Arc<parking_lot::RwLock<Config>> {
        let mut config = Config::default();
        config
            .agents
            .insert(alias.to_string(), AliasedAgentConfig::default());
        Arc::new(parking_lot::RwLock::new(config))
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn same_scope_returns_same_registry_arc() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = fake_mcp_server_script(dir.path(), "kutsu");
        let pool = McpConnectionPool::new(cfg_with_agent("roy", &[("kutsu", &script)]));

        let a = pool.registry_for("roy").await.expect("first checkout");
        let b = pool.registry_for("roy").await.expect("second checkout");
        assert!(
            Arc::ptr_eq(&a, &b),
            "unchanged config + healthy handle must reuse the same registry Arc"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn different_scopes_get_different_registries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let legal_script = fake_mcp_server_script(dir.path(), "kb_legal");
        let hr_script = fake_mcp_server_script(dir.path(), "kb_hr");

        let mut config = Config::default();
        config
            .mcp
            .servers
            .push(stdio_config("kb-legal", &legal_script));
        config.mcp.servers.push(stdio_config("kb-hr", &hr_script));
        config.mcp_bundles.insert(
            "legal-bundle".to_string(),
            McpBundleConfig {
                servers: vec!["kb-legal".to_string()],
                exclude: Vec::new(),
            },
        );
        config.mcp_bundles.insert(
            "hr-bundle".to_string(),
            McpBundleConfig {
                servers: vec!["kb-hr".to_string()],
                exclude: Vec::new(),
            },
        );
        config.agents.insert(
            "kb-legal".to_string(),
            AliasedAgentConfig {
                mcp_bundles: vec!["legal-bundle".to_string()],
                ..AliasedAgentConfig::default()
            },
        );
        config.agents.insert(
            "kb-hr".to_string(),
            AliasedAgentConfig {
                mcp_bundles: vec!["hr-bundle".to_string()],
                ..AliasedAgentConfig::default()
            },
        );
        let pool = McpConnectionPool::new(Arc::new(parking_lot::RwLock::new(config)));

        let legal = pool.registry_for("kb-legal").await.expect("legal scope");
        let hr = pool.registry_for("kb-hr").await.expect("hr scope");
        assert!(
            !Arc::ptr_eq(&legal, &hr),
            "separate scopes must get separate registries (process isolation)"
        );
    }

    #[tokio::test]
    async fn empty_scope_returns_none() {
        let pool = McpConnectionPool::new(cfg_with_bare_agent("bare"));
        assert!(pool.registry_for("bare").await.is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn config_change_rebuilds_that_server() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = fake_mcp_server_script(dir.path(), "kutsu");
        let config = cfg_with_agent("roy", &[("kutsu", &script)]);
        let pool = McpConnectionPool::new(Arc::clone(&config));

        let before = pool.registry_for("roy").await.expect("first checkout");

        // Mutate the scope's server args in the shared Config in place —
        // same server name, different `McpServerConfig`, so the stored
        // config no longer `==` the live one and the reconcile must
        // reconnect rather than reuse the existing handle.
        {
            let mut cfg = config.write();
            if let Some(server) = cfg.mcp.servers.iter_mut().find(|s| s.name == "kutsu") {
                server.args.push("--changed".to_string());
            }
        }

        let after = pool.registry_for("roy").await.expect("second checkout");
        assert!(
            !Arc::ptr_eq(&before, &after),
            "a config change for a server in the scope must rebuild the registry"
        );
    }
}
