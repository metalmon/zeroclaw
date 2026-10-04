//! Fork-local registry of EXTERNAL subjects: people who signed in through an
//! identity provider (`[oidc.<alias>]`, Keycloak today, any OIDC issuer
//! tomorrow) and whom the gateway has admitted at least once.
//!
//! External identities are not configured anywhere: an OIDC login resolves to
//! `IdentitySubject::Oidc { issuer, subject }` and gets its grants from the
//! provider's `profile_map` by group. The panel still needs to show who those
//! people are, so every successful OIDC resolution upserts a non-secret
//! record here (`<data_dir>/authz-external.json`) and
//! `GET /api/authz/external` reads it back. Read-only by design: grants for
//! these subjects are set by the provider's groups, never bound here.
//! Bearers are never stored; a persist failure is logged and never fails the
//! login.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use zeroclaw_api::principal::{AuthenticatedIdentity, IdentitySubject};
use zeroclaw_runtime::security::principal_resolver::OidcMembership;

/// Upper bound on retained subjects; past it the oldest `last_seen` is evicted.
pub const MAX_EXTERNAL_SUBJECTS: usize = 1000;

/// One external subject as seen by the gateway. Wire shape of the entries in
/// `GET /api/authz/external`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ExternalSubjectRecord {
    /// Canonical principal id (`oidc:<issuer-urlencoded>:<sub>`).
    pub id: String,
    /// The `[oidc.<alias>]` provider alias that verified the login.
    pub provider: String,
    pub issuer: String,
    pub subject: String,
    /// `name`, else `preferred_username`, from the verified claims.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Raw values at the provider's `claim_path` on the latest login.
    pub groups: Vec<String>,
    /// Profile aliases `profile_map` assigned to those groups (sorted).
    pub profiles: Vec<String>,
    /// Whether the resolved grants carried `admin` on the latest login.
    pub admin: bool,
    /// RFC 3339, UTC.
    pub first_seen: String,
    /// RFC 3339, UTC.
    pub last_seen: String,
    pub logins: u64,
}

/// In-memory map of external subjects, mirrored to
/// `<data_dir>/authz-external.json` after every change.
#[derive(Debug)]
pub struct ExternalSubjectStore {
    inner: RwLock<HashMap<String, ExternalSubjectRecord>>,
    /// `None` for the in-memory-only (test/ephemeral) store.
    path: Option<PathBuf>,
}

impl Default for ExternalSubjectStore {
    fn default() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            path: None,
        }
    }
}

impl ExternalSubjectStore {
    /// Load `<data_dir>/authz-external.json` (unreadable or malformed means an
    /// empty registry) and persist every change back to it.
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("authz-external.json");
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<ExternalSubjectRecord>>(&s).ok())
            .map(|records| records.into_iter().map(|r| (r.id.clone(), r)).collect())
            .unwrap_or_default();
        Self {
            inner: RwLock::new(map),
            path: Some(path),
        }
    }

    /// In-memory-only store that never persists. For tests and standalone
    /// callers without a `data_dir`.
    #[must_use]
    pub fn new_ephemeral() -> Self {
        Self::default()
    }

    /// Record one successful login of an OIDC human identity. Non-OIDC
    /// subjects (shared operator, roster, service clients) are ignored.
    /// Returns the stored record, if the identity was recordable.
    pub fn record_login(
        &self,
        identity: &AuthenticatedIdentity,
        membership: OidcMembership,
        admin: bool,
    ) -> Option<ExternalSubjectRecord> {
        self.record_login_at(identity, membership, admin, Utc::now())
    }

    fn record_login_at(
        &self,
        identity: &AuthenticatedIdentity,
        membership: OidcMembership,
        admin: bool,
        now: DateTime<Utc>,
    ) -> Option<ExternalSubjectRecord> {
        let IdentitySubject::Oidc { issuer, subject } = &identity.subject else {
            return None;
        };
        let id = identity.subject.principal_id().0;
        let now = now.to_rfc3339_opts(SecondsFormat::Secs, true);
        let claim = |key: &str| {
            identity
                .claims
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let record = {
            let mut map = self.inner.write();
            let record = map
                .entry(id.clone())
                .or_insert_with(|| ExternalSubjectRecord {
                    id: id.clone(),
                    provider: String::new(),
                    issuer: issuer.clone(),
                    subject: subject.clone(),
                    display_name: None,
                    email: None,
                    groups: Vec::new(),
                    profiles: Vec::new(),
                    admin: false,
                    first_seen: now.clone(),
                    last_seen: now.clone(),
                    logins: 0,
                });
            record.provider = identity.provider_alias.clone().unwrap_or_default();
            record.display_name = claim("name").or_else(|| claim("preferred_username"));
            record.email = claim("email");
            record.groups = membership.groups;
            record.profiles = membership.profiles;
            record.admin = admin;
            record.last_seen = now;
            record.logins = record.logins.saturating_add(1);
            let record = record.clone();
            while map.len() > MAX_EXTERNAL_SUBJECTS {
                // ponytail: linear scan on eviction; a last_seen index if the cap ever grows.
                let Some(oldest) = map
                    .values()
                    .min_by(|a, b| a.last_seen.cmp(&b.last_seen).then_with(|| a.id.cmp(&b.id)))
                    .map(|r| r.id.clone())
                else {
                    break;
                };
                map.remove(&oldest);
            }
            self.persist(&map);
            record
        };
        Some(record)
    }

    /// Every seen subject, most recently seen first.
    #[must_use]
    pub fn list(&self) -> Vec<ExternalSubjectRecord> {
        let mut records: Vec<_> = self.inner.read().values().cloned().collect();
        records.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then_with(|| a.id.cmp(&b.id)));
        records
    }

    /// Forget one subject. `false` when it was not there.
    pub fn forget(&self, id: &str) -> bool {
        let mut map = self.inner.write();
        let removed = map.remove(id).is_some();
        if removed {
            self.persist(&map);
        }
        removed
    }

    /// Serialize and atomically replace the file (sibling temp + rename).
    /// Failures are logged, never propagated: a login must not fail because
    /// the registry could not be written.
    fn persist(&self, map: &HashMap<String, ExternalSubjectRecord>) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        let mut records: Vec<_> = map.values().collect();
        records.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then_with(|| a.id.cmp(&b.id)));
        let result = (|| -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_vec_pretty(&records)?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, &json)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            }
            std::fs::rename(&tmp, path)
        })();
        if let Err(error) = result {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({
                        "path": path.display().to_string(),
                        "error": error.to_string(),
                    })),
                "external subject registry could not be persisted; the login proceeds, the registry stays in memory"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;
    use zeroclaw_api::principal::AuthMethod;

    const ISSUER: &str = "https://sso.example.com/realms/main";

    fn identity(sub: &str, claims: serde_json::Value) -> AuthenticatedIdentity {
        let serde_json::Value::Object(claims) = claims else {
            unreachable!()
        };
        AuthenticatedIdentity::new(
            IdentitySubject::Oidc {
                issuer: ISSUER.into(),
                subject: sub.into(),
            },
            AuthMethod::Oidc,
        )
        .with_provider_alias("corp")
        .with_claims(claims)
    }

    fn membership(groups: &[&str], profiles: &[&str]) -> OidcMembership {
        OidcMembership {
            groups: groups.iter().map(|s| s.to_string()).collect(),
            profiles: profiles.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000 + secs, 0).unwrap()
    }

    #[test]
    fn upsert_counts_logins_and_refreshes_claims_but_keeps_first_seen() {
        let store = ExternalSubjectStore::new_ephemeral();
        let first = store
            .record_login_at(
                &identity(
                    "alice",
                    serde_json::json!({ "name": "Alice A", "email": "a@x.io" }),
                ),
                membership(&["ops"], &["operator"]),
                true,
                at(0),
            )
            .expect("oidc identity is recorded");
        assert_eq!(first.id, "oidc:https%3A//sso.example.com/realms/main:alice");
        assert_eq!(first.provider, "corp");
        assert_eq!(first.display_name.as_deref(), Some("Alice A"));
        assert_eq!(first.email.as_deref(), Some("a@x.io"));
        assert_eq!(first.logins, 1);
        assert_eq!(first.first_seen, first.last_seen);

        let second = store
            .record_login_at(
                &identity(
                    "alice",
                    serde_json::json!({ "preferred_username": "alice" }),
                ),
                membership(&["kb"], &["kb"]),
                false,
                at(60),
            )
            .unwrap();
        assert_eq!(second.logins, 2);
        assert_eq!(second.first_seen, first.first_seen);
        assert!(second.last_seen > second.first_seen);
        assert_eq!(second.display_name.as_deref(), Some("alice"));
        assert_eq!(second.email, None);
        assert_eq!(second.groups, vec!["kb"]);
        assert_eq!(second.profiles, vec!["kb"]);
        assert!(!second.admin);
        assert_eq!(store.list().len(), 1);

        assert!(
            store
                .record_login(
                    &AuthenticatedIdentity::shared_operator(AuthMethod::Native),
                    OidcMembership::default(),
                    true,
                )
                .is_none(),
            "only OIDC subjects are external"
        );
    }

    #[test]
    fn list_is_newest_first_and_forget_reports_unknown() {
        let store = ExternalSubjectStore::new_ephemeral();
        for (i, sub) in ["a", "b", "c"].iter().enumerate() {
            store.record_login_at(
                &identity(sub, serde_json::json!({})),
                membership(&[], &[]),
                false,
                at(i as i64 * 10),
            );
        }
        let subjects: Vec<_> = store.list().into_iter().map(|r| r.subject).collect();
        assert_eq!(subjects, vec!["c", "b", "a"]);
        let id_b = "oidc:https%3A//sso.example.com/realms/main:b";
        assert!(store.forget(id_b));
        assert!(!store.forget(id_b));
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn evicts_the_oldest_last_seen_past_the_cap() {
        let store = ExternalSubjectStore::new_ephemeral();
        for i in 0..=MAX_EXTERNAL_SUBJECTS {
            store.record_login_at(
                &identity(&format!("u{i}"), serde_json::json!({})),
                membership(&[], &[]),
                false,
                at(i as i64),
            );
        }
        let list = store.list();
        assert_eq!(list.len(), MAX_EXTERNAL_SUBJECTS);
        assert!(
            !list.iter().any(|r| r.subject == "u0"),
            "the oldest last_seen is evicted"
        );
        assert_eq!(list[0].subject, format!("u{MAX_EXTERNAL_SUBJECTS}"));
    }

    #[test]
    fn persists_and_reloads_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("workspace");
        let store = ExternalSubjectStore::new(&data_dir);
        store.record_login(
            &identity("alice", serde_json::json!({ "email": "a@x.io" })),
            membership(&["ops"], &["operator"]),
            true,
        );
        let on_disk = std::fs::read_to_string(data_dir.join("authz-external.json")).unwrap();
        assert!(on_disk.contains("\"subject\": \"alice\""));
        assert!(!data_dir.join("authz-external.json.tmp").exists());

        let reloaded = ExternalSubjectStore::new(&data_dir);
        let list = reloaded.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].email.as_deref(), Some("a@x.io"));
        assert_eq!(list[0].profiles, vec!["operator"]);
        assert!(list[0].admin);

        assert!(reloaded.forget(&list[0].id));
        assert_eq!(
            std::fs::read_to_string(data_dir.join("authz-external.json"))
                .unwrap()
                .trim(),
            "[]"
        );
    }
}
