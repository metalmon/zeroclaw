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

    /// Named permission profiles (`[[authz.profiles]]`): a reusable bundle of
    /// `allowed_agents` (plus an `admin` escalation flag) that one or more
    /// principals bind to by `id`. Mirrors `principals`'
    /// per-element property routing.
    #[serde(default, rename = "profiles")]
    #[nested]
    #[natural_key = "id"]
    pub profiles: Vec<PermissionProfile>,
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
    /// `[[authz.profiles]]` ids this principal is bound to. Effective access
    /// is the union of every bound profile's `allowed_agents` (see
    /// [`AuthzConfig::effective_agents`]); any bound `admin` profile grants
    /// all agents regardless of the others.
    #[serde(default)]
    pub profiles: Vec<String>,
}

/// A reusable, named bundle of agent access (`[[authz.profiles]]`),
/// referenced by [`PrincipalRecord::profiles`] via `id`. `#[prefix =
/// "authz.profiles"]` mirrors `PrincipalRecord`'s `#[prefix =
/// "authz.principals"]`: the full dotted path is the parent `authz` plus
/// this field's own name `profiles`.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, zeroclaw_macros::Configurable,
)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "authz.profiles"]
pub struct PermissionProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub allowed_agents: Vec<String>,
    /// Grants access to every agent (`["*"]`), overriding `allowed_agents`,
    /// for any principal bound to this profile.
    #[serde(default)]
    pub admin: bool,
}

/// Well-known id for the auto-seeded bootstrap-operator admin principal
/// (the carried ruling: enabling authz must never lock the operator out of
/// the very panel/API that manages roles). See
/// [`AuthzConfig::seed_operator_admin_if_locked_out`] for the seeding logic
/// and its caller-scoping invariant — this id is seeded bound ONLY to the
/// specific credential that performed the enforcement-enabling write, never
/// to every currently-paired token.
///
/// This id alone does not make the operator's connection resolve to this
/// principal in the narrow window between the write that first enables
/// enforcement and the next `/admin/reload`/restart: the gateway's
/// `PairingAuthProvider` (in `zeroclaw-runtime`) holds an `AuthzConfig`
/// SNAPSHOT frozen at daemon start / last reload, never rebuilt on a bare
/// config write, so a brand-new principal id is invisible to
/// `resolve_principal` until then. The gateway crate's `require_admin`
/// closes that specific window with an additional bootstrap-rescue check
/// that re-resolves the CALLER'S OWN bearer against the LIVE config
/// directly (bypassing the stale snapshot entirely, not merely checking
/// "is paired") — see its doc comment
/// (`crates/zeroclaw-gateway/src/api_authz.rs`) for the full story.
pub const OPERATOR_PRINCIPAL_ID: &str = "_operator";

/// Well-known id for the auto-seeded, always-`admin: true` profile
/// [`OPERATOR_PRINCIPAL_ID`] is bound to.
pub const OPERATOR_ADMIN_PROFILE_ID: &str = "_operator_admin";

impl AuthzConfig {
    /// Authz is enforced once any principal or permission profile is configured.
    pub fn is_enforced(&self) -> bool {
        !self.principals.is_empty() || !self.profiles.is_empty()
    }

    /// Find the principal a bearer maps to, by token-hash first then device id.
    /// An empty `token_hash` (device-only credential, no bearer) never matches a
    /// configured `token_hashes` entry — otherwise a misconfigured `[""]` pin
    /// would spuriously match a token-less connection. Device-only credentials
    /// resolve solely via `device_id`.
    pub fn lookup(&self, token_hash: &str, device_id: Option<&str>) -> Option<&PrincipalRecord> {
        self.principals.iter().find(|p| {
            (!token_hash.is_empty() && p.token_hashes.iter().any(|h| h == token_hash))
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

    /// Find a configured profile by its `id`.
    fn profile_by_id(&self, id: &str) -> Option<&PermissionProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// The agents `principal_id` may reach: the union of every bound
    /// profile's `allowed_agents`. If any bound profile has `admin = true`,
    /// short-circuits to `["*"]` (all agents) regardless of the others.
    /// An unknown principal, or one bound to no (or only unknown) profiles,
    /// gets an empty grant — fail-closed.
    #[must_use]
    pub fn effective_agents(&self, principal_id: &str) -> Vec<String> {
        let Some(principal) = self.by_id(principal_id) else {
            return Vec::new();
        };
        let bound_profiles: Vec<&PermissionProfile> = principal
            .profiles
            .iter()
            .filter_map(|id| self.profile_by_id(id))
            .collect();
        if bound_profiles.iter().any(|p| p.admin) {
            return vec!["*".to_string()];
        }
        let mut agents: Vec<String> = Vec::new();
        for profile in bound_profiles {
            for agent in &profile.allowed_agents {
                if !agents.contains(agent) {
                    agents.push(agent.clone());
                }
            }
        }
        agents
    }

    /// True when `principal_id` is bound to any profile with `admin = true`.
    #[must_use]
    pub fn is_admin(&self, principal_id: &str) -> bool {
        let Some(principal) = self.by_id(principal_id) else {
            return false;
        };
        principal
            .profiles
            .iter()
            .filter_map(|id| self.profile_by_id(id))
            .any(|p| p.admin)
    }

    /// Convert-and-clear migration: every principal's legacy inline
    /// `allowed_agents` moves into a generated, per-principal profile
    /// (`_migrated_<principal.id>`), pushed onto `profiles` and bound onto
    /// the principal, and the inline field is cleared. Preserves
    /// [`Self::effective_agents`] behavior for every migrated principal.
    /// Idempotent: a principal with already-empty `allowed_agents` (e.g. a
    /// second call, or one that only ever used `profiles`) is untouched.
    pub fn migrate_inline_agents(&mut self) {
        for principal in &mut self.principals {
            if principal.allowed_agents.is_empty() {
                continue;
            }
            let agents = std::mem::take(&mut principal.allowed_agents);
            let profile_id = format!("_migrated_{}", principal.id);
            match self.profiles.iter_mut().find(|p| p.id == profile_id) {
                Some(existing) => {
                    for agent in agents {
                        if !existing.allowed_agents.contains(&agent) {
                            existing.allowed_agents.push(agent);
                        }
                    }
                }
                None => {
                    self.profiles.push(PermissionProfile {
                        id: profile_id.clone(),
                        allowed_agents: agents,
                        admin: false,
                    });
                }
            }
            if !principal.profiles.contains(&profile_id) {
                principal.profiles.push(profile_id);
            }
        }
    }

    /// Guarantee a locked-out operator credential is never stranded the
    /// instant authz becomes enforced — WITHOUT ever granting admin to a
    /// credential the operator has explicitly configured elsewhere.
    ///
    /// Callers pass exactly the hash(es) that should be considered for
    /// rescue — typically ONE hash, the specific caller who is performing
    /// the config write that turns enforcement on (see
    /// `zeroclaw-gateway`'s `seed_operator_admin_for_caller`). This function
    /// does NOT decide who "the operator" is beyond that: it is the
    /// caller's job to pass only a credential it has independently verified
    /// belongs to whoever is configuring authz right now, never e.g. every
    /// entry in `gateway.paired_tokens` — dumping every already-paired
    /// device in here would silently promote all of them to admin.
    ///
    /// No-op when authz is not enforced (`is_enforced() == false`) — nothing
    /// to protect yet. Otherwise: each given hash is dropped if it is empty
    /// OR if it already resolves (via [`Self::lookup`]) to ANY existing
    /// principal at all, admin or not — a credential the operator has
    /// already explicitly bound to a principal must keep exactly that
    /// principal's permissions and never additionally gain admin through
    /// this path. If nothing survives that filter, this is a no-op. If
    /// anything does, this seeds the well-known [`OPERATOR_ADMIN_PROFILE_ID`]
    /// profile (`admin: true`) and [`OPERATOR_PRINCIPAL_ID`] principal
    /// (bound to it, with the surviving hashes added to its
    /// `token_hashes`) — both edits land in this one call, synchronous, no
    /// yield point in between, so config is never observed with one half
    /// seeded but not the other.
    ///
    /// Idempotent: safe to call repeatedly. An existing
    /// `OPERATOR_ADMIN_PROFILE_ID` / `OPERATOR_PRINCIPAL_ID` is extended in
    /// place (missing hashes/bindings added, nothing duplicated); a hash
    /// already bound to `OPERATOR_PRINCIPAL_ID` itself is excluded by the
    /// same "already resolves to a principal" filter, so a second call with
    /// the same hash changes nothing.
    ///
    /// Returns `true` iff it actually mutated `self` (created or extended
    /// the seed) — `false` for every no-op path. Callers that persist via
    /// `Config::mark_dirty` + `save_dirty` (this module has no persistence
    /// mechanism of its own — see `zeroclaw-gateway`'s
    /// `seed_operator_admin_for_caller`) use this to only mark the affected
    /// paths dirty when something actually changed, so an already-seeded
    /// caller doesn't churn an unnecessary rewrite on every subsequent
    /// config write.
    pub fn seed_operator_admin_if_locked_out(&mut self, candidate_hashes: &[String]) -> bool {
        if !self.is_enforced() {
            return false;
        }
        let hashes: Vec<&str> = candidate_hashes
            .iter()
            .map(String::as_str)
            .filter(|h| !h.is_empty() && self.lookup(h, None).is_none())
            .collect();
        if hashes.is_empty() {
            return false;
        }

        match self
            .profiles
            .iter_mut()
            .find(|p| p.id == OPERATOR_ADMIN_PROFILE_ID)
        {
            Some(existing) => existing.admin = true,
            None => self.profiles.push(PermissionProfile {
                id: OPERATOR_ADMIN_PROFILE_ID.to_string(),
                allowed_agents: Vec::new(),
                admin: true,
            }),
        }

        match self
            .principals
            .iter_mut()
            .find(|p| p.id == OPERATOR_PRINCIPAL_ID)
        {
            Some(existing) => {
                if !existing
                    .profiles
                    .iter()
                    .any(|id| id == OPERATOR_ADMIN_PROFILE_ID)
                {
                    existing
                        .profiles
                        .push(OPERATOR_ADMIN_PROFILE_ID.to_string());
                }
                for h in hashes.iter().copied() {
                    if !existing.token_hashes.iter().any(|x| x == h) {
                        existing.token_hashes.push(h.to_string());
                    }
                }
            }
            None => self.principals.push(PrincipalRecord {
                id: OPERATOR_PRINCIPAL_ID.to_string(),
                allowed_agents: Vec::new(),
                device_ids: Vec::new(),
                token_hashes: hashes.iter().copied().map(str::to_string).collect(),
                profiles: vec![OPERATOR_ADMIN_PROFILE_ID.to_string()],
            }),
        }
        true
    }

    /// Read-only diagnostic for callers that have NO specific caller to
    /// scope a seed to — chiefly the config-load pipeline, which parses a
    /// file and has no request/bearer at all. True when authz is enforced,
    /// at least one non-empty hash is given, and NONE of `paired_hashes`
    /// resolves (via [`Self::lookup`]) to an admin principal — i.e. every
    /// currently-paired credential would be denied by the gateway's
    /// `require_admin`.
    ///
    /// Deliberately does NOT drive an automatic seed: guessing which of
    /// potentially several paired tokens is "the operator's" during a bare
    /// config load, with no caller to attribute the grant to, is exactly
    /// the amplification this module's seeding function refuses to do (see
    /// [`Self::seed_operator_admin_if_locked_out`]'s caller-scoping
    /// invariant). Callers should log a warning on `true` and leave the fix
    /// to the operator (who has file/API access to bind an existing
    /// principal to an admin profile, or to reconnect through the live
    /// gateway, which CAN scope a seed to that specific reconnecting
    /// caller).
    #[must_use]
    pub fn bootstrap_operator_is_locked_out(&self, paired_hashes: &[String]) -> bool {
        if !self.is_enforced() {
            return false;
        }
        let hashes: Vec<&str> = paired_hashes
            .iter()
            .map(String::as_str)
            .filter(|h| !h.is_empty())
            .collect();
        if hashes.is_empty() {
            return false;
        }
        !hashes
            .iter()
            .copied()
            .any(|h| self.lookup(h, None).is_some_and(|p| self.is_admin(&p.id)))
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
    fn lookup_empty_token_hash_never_matches_a_pin() {
        // A device-only (token-less) credential passes "" as the token hash.
        // Even a misconfigured empty `token_hashes` pin must NOT match it;
        // resolution for such a credential is device_id-only.
        let c: AuthzConfig = toml::from_str(
            r#"
            [[principals]]
            id = "x"
            allowed_agents = ["bot"]
            token_hashes = [""]
            device_ids = ["dev-x"]
            "#,
        )
        .unwrap();
        assert!(c.lookup("", None).is_none());
        assert_eq!(c.lookup("", Some("dev-x")).unwrap().id, "x");
    }

    #[test]
    fn by_id_finds_configured_principal() {
        let c = cfg();
        assert_eq!(c.by_id("alice").unwrap().allowed_agents, vec!["crm-bot"]);
        assert_eq!(c.by_id("admin").unwrap().allowed_agents, vec!["*"]);
        assert!(c.by_id("ghost").is_none());
    }

    /// Regression for the `Configurable` derive's `create_map_key` element
    /// seeding: a `#[natural_key = "id"]` Vec section (`authz.principals` /
    /// `authz.profiles`) must write the supplied key into the element's ACTUAL
    /// `id` field — not the historically hardcoded `name`/`hint`, which these
    /// structs don't even have. A blank `id` silently corrupts the row: every
    /// later `set_prop("authz.<sec>.<id>.<field>")` fails to resolve because no
    /// element's `id` matches the alias. Guards the fix that replaced the
    /// gateway's `fixup_created_*_natural_key` post-creation patches.
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

        let created = c
            .create_map_key("authz.profiles", "crm")
            .expect("authz.profiles must accept a new element");
        assert!(created);
        assert_eq!(c.profiles.len(), 1);
        assert_eq!(
            c.profiles[0].id, "crm",
            "the created profile's `id` natural key must be seeded from the map key"
        );

        // The seeded id must route: a follow-up field write on the created
        // element's dotted path resolves (the exact sequence the api_authz
        // handlers run right after create_map_key). A blank id would 404 here.
        c.set_prop("authz.profiles.crm.admin", "true")
            .expect("set_prop on the created profile must resolve via its seeded id");
        assert!(c.profiles[0].admin);
    }

    #[test]
    fn effective_agents_unions_bound_profiles() {
        let c = AuthzConfig {
            profiles: vec![
                PermissionProfile {
                    id: "crm".into(),
                    allowed_agents: vec!["crm-bot".into()],
                    admin: false,
                },
                PermissionProfile {
                    id: "hr".into(),
                    allowed_agents: vec!["hr-bot".into()],
                    admin: false,
                },
            ],
            principals: vec![PrincipalRecord {
                id: "u1".into(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec![],
                profiles: vec!["crm".into(), "hr".into()],
            }],
        };
        let mut got = c.effective_agents("u1");
        got.sort();
        assert_eq!(got, vec!["crm-bot".to_string(), "hr-bot".into()]);
    }

    #[test]
    fn admin_profile_grants_all_even_when_agents_empty() {
        let c = AuthzConfig {
            profiles: vec![PermissionProfile {
                id: "ops".into(),
                allowed_agents: vec![],
                admin: true,
            }],
            principals: vec![PrincipalRecord {
                id: "a".into(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec![],
                profiles: vec!["ops".into()],
            }],
        };
        assert_eq!(c.effective_agents("a"), vec!["*".to_string()]);
        assert!(c.is_admin("a"));
    }

    #[test]
    fn migrate_moves_inline_agents_into_a_profile_and_clears() {
        let mut c = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "legacy".into(),
                allowed_agents: vec!["crm-bot".into()],
                device_ids: vec![],
                token_hashes: vec![],
                profiles: vec![],
            }],
            profiles: vec![],
        };
        c.migrate_inline_agents();
        // inline cleared, a generated profile now referenced, behavior preserved
        assert!(c.principals[0].allowed_agents.is_empty());
        assert_eq!(c.effective_agents("legacy"), vec!["crm-bot".to_string()]);
    }

    #[test]
    fn seed_operator_admin_noop_when_not_enforced() {
        let mut c = AuthzConfig::default();
        c.seed_operator_admin_if_locked_out(&["hash1".to_string()]);
        assert!(!c.is_enforced(), "must not itself enforce authz");
        assert!(c.principals.is_empty());
        assert!(c.profiles.is_empty());
    }

    #[test]
    fn seed_operator_admin_noop_when_no_bootstrap_hashes() {
        let mut c = cfg(); // enforced (alice + admin principals from the fixture)
        let before = c.clone();
        c.seed_operator_admin_if_locked_out(&[]);
        assert_eq!(c, before, "no bootstrap hashes means nothing to protect");

        c.seed_operator_admin_if_locked_out(&[String::new()]);
        assert_eq!(
            c, before,
            "an empty-string hash must never count as a real bootstrap credential"
        );
    }

    #[test]
    fn seed_operator_admin_noop_when_bootstrap_already_admin() {
        let mut c = admin_authz_fixture("bob-hash", "bob");
        let before = c.clone();
        c.seed_operator_admin_if_locked_out(&["bob-hash".to_string()]);
        assert_eq!(
            c, before,
            "the bootstrap hash already resolves to an admin principal — nothing to seed"
        );
    }

    #[test]
    fn seed_operator_admin_creates_the_well_known_principal_when_locked_out() {
        // Enforced (via a non-admin profile), but the bootstrap hash doesn't
        // map to anything — exactly the lock-out this seed exists to avoid.
        let mut c = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec!["alice-hash".into()],
                profiles: vec!["crm".into()],
            }],
            profiles: vec![PermissionProfile {
                id: "crm".into(),
                allowed_agents: vec!["crm-bot".into()],
                admin: false,
            }],
        };
        c.seed_operator_admin_if_locked_out(&["operator-hash".to_string()]);

        assert!(c.is_admin(OPERATOR_PRINCIPAL_ID));
        let op = c.by_id(OPERATOR_PRINCIPAL_ID).expect("operator seeded");
        assert_eq!(op.token_hashes, vec!["operator-hash".to_string()]);
        assert_eq!(op.profiles, vec![OPERATOR_ADMIN_PROFILE_ID.to_string()]);
        let profile = c
            .profiles
            .iter()
            .find(|p| p.id == OPERATOR_ADMIN_PROFILE_ID)
            .expect("operator admin profile seeded");
        assert!(profile.admin);
        // The pre-existing principal/profile must survive untouched.
        assert!(c.by_id("alice").is_some());
        assert_eq!(
            c.lookup("operator-hash", None).unwrap().id,
            OPERATOR_PRINCIPAL_ID
        );
    }

    #[test]
    fn seed_operator_admin_is_idempotent() {
        let mut c = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec!["alice-hash".into()],
                profiles: vec![],
            }],
            profiles: vec![],
        };
        c.seed_operator_admin_if_locked_out(&["operator-hash".to_string()]);
        let after_first = c.clone();
        c.seed_operator_admin_if_locked_out(&["operator-hash".to_string()]);
        assert_eq!(
            c, after_first,
            "a second identical call must change nothing"
        );
        assert_eq!(
            c.principals
                .iter()
                .filter(|p| p.id == OPERATOR_PRINCIPAL_ID)
                .count(),
            1
        );
        assert_eq!(
            c.by_id(OPERATOR_PRINCIPAL_ID).unwrap().token_hashes,
            vec!["operator-hash".to_string()],
            "the hash must not be duplicated"
        );
    }

    #[test]
    fn seed_operator_admin_extends_an_existing_operator_principal_with_a_new_hash() {
        // A prior seed run (e.g. an earlier daemon boot with a different
        // paired-token set) already created the operator principal; a new
        // bootstrap hash must be added, not clobber the old one.
        let mut c = AuthzConfig {
            principals: vec![
                PrincipalRecord {
                    id: "alice".into(),
                    allowed_agents: vec![],
                    device_ids: vec![],
                    token_hashes: vec!["alice-hash".into()],
                    profiles: vec![],
                },
                PrincipalRecord {
                    id: OPERATOR_PRINCIPAL_ID.into(),
                    allowed_agents: vec![],
                    device_ids: vec![],
                    token_hashes: vec!["old-hash".into()],
                    profiles: vec![OPERATOR_ADMIN_PROFILE_ID.into()],
                },
            ],
            profiles: vec![PermissionProfile {
                id: OPERATOR_ADMIN_PROFILE_ID.into(),
                allowed_agents: vec![],
                admin: true,
            }],
        };
        c.seed_operator_admin_if_locked_out(&["new-hash".to_string()]);

        let op = c.by_id(OPERATOR_PRINCIPAL_ID).unwrap();
        assert_eq!(
            op.token_hashes,
            vec!["old-hash".to_string(), "new-hash".to_string()]
        );
        assert_eq!(
            c.principals
                .iter()
                .filter(|p| p.id == OPERATOR_PRINCIPAL_ID)
                .count(),
            1,
            "must extend in place, never duplicate the principal"
        );
    }

    #[test]
    fn seed_operator_admin_excludes_a_hash_already_bound_to_another_principal() {
        // Bob's hash is explicitly configured against a non-admin "guest"
        // principal. Even though authz is enforced and Bob's hash is passed
        // as a candidate, it must NOT be pulled into `_operator` — an
        // already-configured credential keeps exactly its own principal's
        // permissions, never gains admin as a side effect. This is the
        // exact amplification the seed must never cause.
        let mut c = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "guest".into(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec!["bob-hash".into()],
                profiles: vec![],
            }],
            profiles: vec![],
        };
        let before = c.clone();
        c.seed_operator_admin_if_locked_out(&["bob-hash".to_string()]);
        assert_eq!(
            c, before,
            "a hash already bound to another (even non-admin) principal \
             must never be pulled into the operator seed"
        );
        assert!(c.by_id(OPERATOR_PRINCIPAL_ID).is_none());
        assert!(!c.is_admin("guest"));
    }

    #[test]
    fn seed_operator_admin_seeds_only_the_unbound_hash_among_a_mixed_set() {
        // A realistic multi-candidate call: one hash is already bound
        // elsewhere, the other is genuinely unbound. Only the unbound one
        // may land in `_operator`; the bound one's own principal must stay
        // exactly as configured (non-admin).
        let mut c = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "guest".into(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec!["bob-hash".into()],
                profiles: vec![],
            }],
            profiles: vec![],
        };
        c.seed_operator_admin_if_locked_out(&["bob-hash".to_string(), "operator-hash".to_string()]);
        let op = c
            .by_id(OPERATOR_PRINCIPAL_ID)
            .expect("operator seeded from the unbound hash");
        assert_eq!(op.token_hashes, vec!["operator-hash".to_string()]);
        assert!(!c.is_admin("guest"));
    }

    #[test]
    fn bootstrap_operator_is_locked_out_true_when_no_paired_hash_is_admin() {
        let c = admin_authz_fixture("bob-hash", "bob");
        // A different, unrelated paired hash than the configured admin's.
        assert!(c.bootstrap_operator_is_locked_out(&["someone-else-hash".to_string()]));
    }

    #[test]
    fn bootstrap_operator_is_locked_out_false_when_a_paired_hash_is_admin() {
        let c = admin_authz_fixture("bob-hash", "bob");
        assert!(!c.bootstrap_operator_is_locked_out(&["bob-hash".to_string()]));
    }

    #[test]
    fn bootstrap_operator_is_locked_out_false_when_not_enforced() {
        let c = AuthzConfig::default();
        assert!(!c.bootstrap_operator_is_locked_out(&["any-hash".to_string()]));
    }

    #[test]
    fn bootstrap_operator_is_locked_out_false_with_no_paired_hashes() {
        let c = admin_authz_fixture("bob-hash", "bob");
        assert!(!c.bootstrap_operator_is_locked_out(&[]));
    }

    /// Fixture: a single principal `principal_id` bound to an `admin: true`
    /// profile, pinned to `token_hash`.
    fn admin_authz_fixture(token_hash: &str, principal_id: &str) -> AuthzConfig {
        AuthzConfig {
            principals: vec![PrincipalRecord {
                id: principal_id.to_string(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec![token_hash.to_string()],
                profiles: vec!["ops".to_string()],
            }],
            profiles: vec![PermissionProfile {
                id: "ops".to_string(),
                allowed_agents: vec![],
                admin: true,
            }],
        }
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
