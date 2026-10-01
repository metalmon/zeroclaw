//! Fork-local principal-tagged pairing (`[[authz.principals]]`): a bearer
//! token bound to a durable `principal_id`, feeding the daemon's
//! `PairingAuthProvider` (`zeroclaw-runtime`) and, via
//! `PrincipalResolver`'s roster merge, upstream's RFC 7141 identity/grants
//! pipeline (`zeroclaw-api::principal`, `zeroclaw-api::grants`).
//!
//! This module carries ONLY the token→principal_id half of the picture.
//! Which agents/resources a `principal_id` may use is upstream's
//! `[permission_profiles.<alias>]` vocabulary (`PrincipalRecord::profiles`
//! names those aliases directly) — this module never computes grants itself.
//!
//! Dropped relative to the pre-upstream-RBAC fork original: `device_ids`/mTLS
//! binding (Chromium/WebView2 clients can't present a client cert; no
//! deployment needs cert identity — see `_local/reconcile-f4-vs-upstream-stage5.md`),
//! the inline `allowed_agents` field and its migration (superseded by
//! `[permission_profiles.<alias>]`), and the operator-bootstrap-rescue
//! seeding (`seed_operator_admin_if_locked_out`) — out of scope for this
//! re-host pass; the gateway's existing `require_admin` bootstrap path is
//! unaffected since it predates and is independent of this module.

use serde::{Deserialize, Serialize};

/// Fork-local principal → bound-profile map (`[[authz.principals]]`).
/// Absent/empty ⇒ authz not enforced (legacy shared-operator pairing,
/// unchanged from today).
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

/// One fork-local principal: a bearer identity (token hash) bound to the
/// `[permission_profiles.<alias>]` names it is entitled to. `#[prefix =
/// "authz.principals"]` is the full dotted path (parent `authz` + this
/// field's own name `principals`), matching `McpServerConfig`'s `#[prefix =
/// "mcp.servers"]`. Every field carries `#[serde(default)]` so
/// `create_map_key` can default-construct a blank element from `{}` before
/// the natural key (`id`) is injected via `set_prop`.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, zeroclaw_macros::Configurable,
)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "authz.principals"]
pub struct PrincipalRecord {
    /// Durable principal id. Feeds `IdentitySubject::Roster { principal_id
    /// }` (`zeroclaw-api::principal`) — must be stable across reloads/
    /// restarts, same contract as `[users.<name>]`'s `effective_principal_id`.
    #[serde(default)]
    pub id: String,
    /// SHA-256 hex digests of bearer tokens bound to this principal
    /// (`PairingGuard::token_hash` shape). A token's hash matching an entry
    /// here — or a live [`TokenBindingStore`] binding naming this `id` — is
    /// what lets `PairingAuthProvider` emit `IdentitySubject::Roster{ principal_id:
    /// id }` for it.
    #[serde(default)]
    pub token_hashes: Vec<String>,
    /// `[permission_profiles.<alias>]` names this principal is bound to.
    /// Merged into `PrincipalResolver`'s roster
    /// (`crates/zeroclaw-runtime/src/security/principal_resolver.rs`)
    /// alongside `[users.<name>].permission_profiles` — same namespace, same
    /// deny-by-default/fail-closed-on-conflict semantics.
    #[serde(default)]
    pub profiles: Vec<String>,
}

impl AuthzConfig {
    /// Authz pairing is enforced once any principal is configured.
    pub fn is_enforced(&self) -> bool {
        !self.principals.is_empty()
    }

    /// Find the principal a bearer token hash maps to via the INLINE
    /// `token_hashes` config pin. An empty hash never matches — otherwise a
    /// misconfigured `[""]` pin would spuriously match a token-less
    /// credential.
    pub fn lookup(&self, token_hash: &str) -> Option<&PrincipalRecord> {
        if token_hash.is_empty() {
            return None;
        }
        self.principals
            .iter()
            .find(|p| p.token_hashes.iter().any(|h| h == token_hash))
    }

    /// Find a configured principal by its `id`. Used by the runtime binding
    /// store path: a token bound in [`TokenBindingStore`] names a principal
    /// id, which must still resolve to a configured record (grants live in
    /// config, via the resolver's roster merge). A binding whose id is
    /// absent here → fail-closed Denied at the provider.
    pub fn by_id(&self, id: &str) -> Option<&PrincipalRecord> {
        self.principals.iter().find(|p| p.id == id)
    }
}

/// Live, persistent `token_hash -> principal_id` binding store rooted at
/// `data_dir`. This is the runtime half of the token→principal separation of
/// concerns: **config** carries who-can-do-what (`PrincipalRecord::id` +
/// `profiles`); **this store** carries which-token-is-which-principal,
/// written when a principal-tagged pairing code is redeemed and read by the
/// connection auth provider on the very next connect (no reload).
///
/// Fail-closed is preserved upstream: a binding here only grants access if
/// the named `principal_id` still resolves via [`AuthzConfig::by_id`] AND
/// the shared `PrincipalResolver`'s roster. Removing the principal from
/// config revokes every token bound to it.
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
    /// this process.
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

    /// Remove any binding for `token_hash` and persist. Companion to every
    /// single-token revocation (`PairingGuard::revoke_token_hash`): a
    /// binding only means something for a paired token. No-op if the token
    /// was not bound.
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

    /// Drop every binding and persist the (now empty) map. Companion to
    /// `PairingGuard::revoke_all_tokens`: once no token is paired, no
    /// binding may keep naming one. No-op on an already-empty store.
    pub fn clear(&self) -> anyhow::Result<()> {
        let snapshot = {
            let mut guard = self
                .inner
                .write()
                .expect("token binding store lock poisoned");
            guard.clear();
            guard.clone()
        };
        self.persist(&snapshot)
    }

    /// Remove every binding that names `principal_id` and persist. Returns
    /// the token hashes that were bound so the caller can revoke the
    /// matching bearers from the pairing guard. Nothing is written when no
    /// binding named the principal.
    pub fn remove_principal(&self, principal_id: &str) -> anyhow::Result<Vec<String>> {
        let (removed, snapshot) = {
            let mut guard = self
                .inner
                .write()
                .expect("token binding store lock poisoned");
            let mut removed = Vec::new();
            guard.retain(|hash, bound| {
                if bound == principal_id {
                    removed.push(hash.clone());
                    return false;
                }
                true
            });
            (removed, guard.clone())
        };
        if !removed.is_empty() {
            self.persist(&snapshot)?;
        }
        Ok(removed)
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
            token_hashes = ["abc123"]
            profiles = ["crm"]
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
        assert_eq!(c.lookup("abc123").unwrap().id, "alice");
        assert!(c.lookup("nope").is_none());
    }

    #[test]
    fn lookup_empty_token_hash_never_matches_a_pin() {
        let c: AuthzConfig = toml::from_str(
            r#"
            [[principals]]
            id = "x"
            token_hashes = [""]
            "#,
        )
        .unwrap();
        assert!(c.lookup("").is_none());
    }

    #[test]
    fn by_id_finds_configured_principal() {
        let c = cfg();
        assert_eq!(c.by_id("alice").unwrap().profiles, vec!["crm".to_string()]);
        assert!(c.by_id("ghost").is_none());
    }

    /// Regression for the `Configurable` derive's `create_map_key` element
    /// seeding: a `#[natural_key = "id"]` Vec section (`authz.principals`)
    /// must write the supplied key into the element's ACTUAL `id` field —
    /// not the historically hardcoded `name`/`hint`, which this struct
    /// doesn't even have. A blank `id` silently corrupts the row: every
    /// later `set_prop("authz.principals.<id>.<field>")` fails to resolve
    /// because no element's `id` matches the alias.
    #[test]
    fn create_map_key_seeds_id_natural_key_and_round_trips() {
        let mut c = AuthzConfig::default();

        let created = c
            .create_map_key("authz.principals", "alice")
            .expect("authz.principals must accept a new element");
        assert!(created, "a brand-new key must report Ok(true)");
        assert_eq!(c.principals.len(), 1);
        assert_eq!(
            c.principals[0].id, "alice",
            "the created principal's `id` natural key must be seeded from the map key"
        );

        c.set_prop("authz.principals.alice.profiles", r#"["crm"]"#)
            .expect("set_prop on the created principal must resolve via its seeded id");
        assert_eq!(c.principals[0].profiles, vec!["crm".to_string()]);
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
    fn binding_store_remove_principal_drops_only_its_bindings() {
        let store = TokenBindingStore::new_ephemeral();
        store.set("h1".into(), "alice".into()).unwrap();
        store.set("h2".into(), "alice".into()).unwrap();
        store.set("h3".into(), "bob".into()).unwrap();
        let mut removed = store.remove_principal("alice").unwrap();
        removed.sort();
        assert_eq!(removed, vec!["h1".to_string(), "h2".to_string()]);
        assert!(store.get("h1").is_none());
        assert!(store.get("h2").is_none());
        assert_eq!(store.get("h3").as_deref(), Some("bob"));
        assert!(store.remove_principal("nobody").unwrap().is_empty());
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
    fn binding_store_clear_drops_every_binding_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenBindingStore::new(dir.path());
        store.set("h1".into(), "alice".into()).unwrap();
        store.set("h2".into(), "bob".into()).unwrap();
        store.clear().unwrap();
        assert!(store.get("h1").is_none());
        assert!(store.get("h2").is_none());
        // The empty map reached disk: a fresh store over the same dir is empty.
        let reloaded = TokenBindingStore::new(dir.path());
        assert!(reloaded.get("h1").is_none());
        assert!(reloaded.get("h2").is_none());
    }

    #[test]
    fn binding_store_new_on_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
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
        let reopened = TokenBindingStore::new(dir.path());
        assert_eq!(reopened.get("h1").as_deref(), Some("alice"));
    }
}
