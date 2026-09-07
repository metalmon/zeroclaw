use serde::{Deserialize, Serialize};

/// Fork-local principal → allowed-agents map (`[[authz.principals]]`).
/// Absent/empty ⇒ authz not enforced (legacy shared-operator).
///
/// Mirrors the `[mcp]` / `[[mcp.servers]]` shape: this wrapper struct
/// derives `Configurable` with `#[prefix = "authz"]`, and its `principals`
/// Vec is `#[nested]` + `#[natural_key = "id"]` so each element is a real
/// `Configurable` child (see `PrincipalRecord`) reachable at
/// `authz.principals.<id>.<field>` instead of a single opaque leaf prop.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, zeroclaw_macros::Configurable,
)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "authz"]
pub struct AuthzConfig {
    /// Configured principals. `#[natural_key = "id"]` opts the Vec into
    /// per-element property routing (`authz.principals.<id>.<field>`),
    /// matching `mcp.servers` / `model_routes`.
    #[serde(default, rename = "principals")]
    #[nested]
    #[natural_key = "id"]
    pub principals: Vec<PrincipalRecord>,
}

/// One fork-local principal: the bearer identity (token hash / device id)
/// mapped to the set of agent aliases it may reach. `#[prefix =
/// "authz.principals"]` is the full dotted path (parent `authz` +
/// this field's own name `principals`), matching `McpServerConfig`'s
/// `#[prefix = "mcp.servers"]`. Every field carries `#[serde(default)]` so
/// `create_map_key` can default-construct a blank element from `{}` before
/// the natural key (`id`) is injected via `set_prop`.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, zeroclaw_macros::Configurable,
)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "authz.principals"]
pub struct PrincipalRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub allowed_agents: Vec<String>,
    #[serde(default)]
    pub device_ids: Vec<String>,
    #[serde(default)]
    pub token_hashes: Vec<String>,
}

impl AuthzConfig {
    /// Authz is enforced once any principal is configured.
    pub fn is_enforced(&self) -> bool {
        !self.principals.is_empty()
    }

    /// Find the principal a bearer maps to, by token-hash first then device id.
    pub fn lookup(&self, token_hash: &str, device_id: Option<&str>) -> Option<&PrincipalRecord> {
        self.principals.iter().find(|p| {
            p.token_hashes.iter().any(|h| h == token_hash)
                || device_id.is_some_and(|d| p.device_ids.iter().any(|x| x == d))
        })
    }

    /// Find a configured principal by its `id`. Used by the runtime binding
    /// store path: a token bound in [`TokenBindingStore`] names a principal id,
    /// which must still resolve to a configured record (permissions live in
    /// config). A binding whose id is absent here → fail-closed Denied.
    pub fn by_id(&self, id: &str) -> Option<&PrincipalRecord> {
        self.principals.iter().find(|p| p.id == id)
    }
}

/// Live, persistent `token_hash -> principal_id` binding store rooted at
/// `data_dir`. This is the runtime half of F4a's separation of concerns:
/// **config** carries who-can-do-what (principal `id` + `allowed_agents`);
/// **this store** carries which-token-is-which-principal, written when a
/// `--principal`-tagged pairing code is redeemed and read by the connection
/// auth provider on the very next connect (no reload).
///
/// Fail-closed is preserved upstream: a binding here only grants access if the
/// named `principal_id` still resolves via [`AuthzConfig::by_id`]. Removing the
/// principal from config revokes every token bound to it.
///
/// Persistence is a flat JSON object (`{"<token_hash>": "<principal_id>"}`) at
/// `<data_dir>/authz-bindings.json`. It holds only hashes and ids — no secret
/// material — so a plain file is acceptable; on Unix we still best-effort a
/// `0600` mode. A missing or corrupt file loads as an empty store and never
/// errors the daemon.
#[derive(Debug)]
pub struct TokenBindingStore {
    inner: std::sync::RwLock<std::collections::HashMap<String, String>>,
    /// `None` for the in-memory-only (test/ephemeral) store: `set` never
    /// touches disk.
    path: Option<std::path::PathBuf>,
}

impl Default for TokenBindingStore {
    /// In-memory-only store (no persistence). For tests and callers that do not
    /// have a `data_dir`.
    fn default() -> Self {
        Self {
            inner: std::sync::RwLock::new(std::collections::HashMap::new()),
            path: None,
        }
    }
}

impl TokenBindingStore {
    /// Open (or lazily create) the store rooted at `data_dir`. Loads the
    /// existing bindings file if present; a missing, empty, or corrupt file
    /// yields an empty store — never an error (the daemon must not fail to
    /// start because this file is unreadable).
    #[must_use]
    pub fn new(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("authz-bindings.json");
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| {
                serde_json::from_str::<std::collections::HashMap<String, String>>(&s).ok()
            })
            .unwrap_or_default();
        Self {
            inner: std::sync::RwLock::new(map),
            path: Some(path),
        }
    }

    /// In-memory-only store that never persists. Convenience for tests.
    #[must_use]
    pub fn new_ephemeral() -> Self {
        Self::default()
    }

    /// The principal id currently bound to `token_hash`, if any.
    #[must_use]
    pub fn get(&self, token_hash: &str) -> Option<String> {
        self.inner
            .read()
            .expect("token binding store lock poisoned")
            .get(token_hash)
            .cloned()
    }

    /// Bind `token_hash` to `principal_id`, overwriting any prior binding, and
    /// persist the whole map. Idempotent: re-binding the same pair is a no-op
    /// write. Errors only reflect disk failures; the in-memory binding is
    /// updated regardless so a caller that ignores the error still resolves in
    /// this process (the `/pair` path treats a persist failure as WARN-only).
    pub fn set(&self, token_hash: String, principal_id: String) -> anyhow::Result<()> {
        let snapshot = {
            let mut guard = self
                .inner
                .write()
                .expect("token binding store lock poisoned");
            guard.insert(token_hash, principal_id);
            guard.clone()
        };
        self.persist(&snapshot)
    }

    /// Remove any binding for `token_hash` and persist. For future token
    /// revocation. No-op if the token was not bound.
    pub fn remove(&self, token_hash: &str) -> anyhow::Result<()> {
        let snapshot = {
            let mut guard = self
                .inner
                .write()
                .expect("token binding store lock poisoned");
            guard.remove(token_hash);
            guard.clone()
        };
        self.persist(&snapshot)
    }

    /// Serialize `map` and atomically replace the bindings file (temp + rename).
    /// In-memory-only stores (`path == None`) skip disk entirely.
    fn persist(&self, map: &std::collections::HashMap<String, String>) -> anyhow::Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(map)?;
        // Atomic replace via a sibling temp file + rename, using only `std::fs`
        // (no `tempfile` prod dependency). `rename` over the same directory is
        // atomic on POSIX and replaces the target on Windows.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AuthzConfig {
        toml::from_str(
            r#"
            [[principals]]
            id = "alice"
            allowed_agents = ["crm-bot"]
            token_hashes = ["abc123"]

            [[principals]]
            id = "admin"
            allowed_agents = ["*"]
            device_ids = ["dev-1"]
            "#,
        )
        .unwrap()
    }

    #[test]
    fn empty_config_is_not_enforced() {
        assert!(!AuthzConfig::default().is_enforced());
    }

    #[test]
    fn nonempty_config_is_enforced() {
        assert!(cfg().is_enforced());
    }

    #[test]
    fn lookup_by_token_hash() {
        let c = cfg();
        assert_eq!(c.lookup("abc123", None).unwrap().id, "alice");
        assert!(c.lookup("nope", None).is_none());
    }

    #[test]
    fn lookup_by_device_id() {
        let c = cfg();
        assert_eq!(c.lookup("other", Some("dev-1")).unwrap().id, "admin");
    }

    #[test]
    fn by_id_finds_configured_principal() {
        let c = cfg();
        assert_eq!(c.by_id("alice").unwrap().allowed_agents, vec!["crm-bot"]);
        assert_eq!(c.by_id("admin").unwrap().allowed_agents, vec!["*"]);
        assert!(c.by_id("ghost").is_none());
    }

    #[test]
    fn binding_store_set_get_round_trip() {
        let store = TokenBindingStore::new_ephemeral();
        assert!(store.get("h1").is_none());
        store.set("h1".into(), "alice".into()).unwrap();
        assert_eq!(store.get("h1").as_deref(), Some("alice"));
    }

    #[test]
    fn binding_store_get_missing_is_none() {
        let store = TokenBindingStore::new_ephemeral();
        assert!(store.get("nope").is_none());
    }

    #[test]
    fn binding_store_rebind_overwrites() {
        let store = TokenBindingStore::new_ephemeral();
        store.set("h1".into(), "alice".into()).unwrap();
        store.set("h1".into(), "bob".into()).unwrap();
        assert_eq!(store.get("h1").as_deref(), Some("bob"));
    }

    #[test]
    fn binding_store_remove_clears_binding() {
        let store = TokenBindingStore::new_ephemeral();
        store.set("h1".into(), "alice".into()).unwrap();
        store.remove("h1").unwrap();
        assert!(store.get("h1").is_none());
    }

    #[test]
    fn binding_store_new_on_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        // No file written yet.
        let store = TokenBindingStore::new(dir.path());
        assert!(store.get("anything").is_none());
    }

    #[test]
    fn binding_store_new_on_corrupt_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("authz-bindings.json"), b"{not json").unwrap();
        let store = TokenBindingStore::new(dir.path());
        assert!(store.get("anything").is_none());
    }

    #[test]
    fn binding_store_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = TokenBindingStore::new(dir.path());
            store.set("h1".into(), "alice".into()).unwrap();
        }
        // A fresh store rooted at the same data_dir reads the prior write.
        let reopened = TokenBindingStore::new(dir.path());
        assert_eq!(reopened.get("h1").as_deref(), Some("alice"));
    }
}
