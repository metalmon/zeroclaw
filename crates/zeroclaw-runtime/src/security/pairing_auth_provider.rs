//! [`AuthProvider`] that maps a paired bearer token to a [`Principal`] via
//! `[[authz.principals]]` (fork-local principal → allowed-agents map).
//!
//! Fail-closed contract: once any principal is configured (`authz.is_enforced()`),
//! an unmapped bearer is `Denied { BadCredential }` — never a silent allow. When no
//! principal is configured (legacy/default config), any non-empty bearer is
//! `Trusted` as the [`Principal::shared_operator`] sentinel, preserving today's
//! single-operator behaviour unchanged.

use std::sync::Arc;

use async_trait::async_trait;
use zeroclaw_api::principal::{
    AgentAlias, AuthMethod, AuthOutcome, DenyReason, Principal, PrincipalId,
};
use zeroclaw_config::authz::{AuthzConfig, PrincipalRecord, TokenBindingStore};
use zeroclaw_config::pairing::PairingGuard;

use super::auth_provider::{AuthProvider, Credential};

/// Maps a paired bearer token to a `Principal` via `[[authz.principals]]` plus
/// the live [`TokenBindingStore`]. Fail-closed when authz is enforced; legacy
/// shared-operator otherwise.
pub struct PairingAuthProvider {
    authz: AuthzConfig,
    /// Live, shared runtime binding store: `token_hash -> principal_id` written
    /// when a `--principal`-tagged code is redeemed at `/pair`. Shared `Arc`
    /// with the gateway so a freshly-paired token resolves on the next connect
    /// with no reload.
    bindings: Arc<TokenBindingStore>,
}

impl PairingAuthProvider {
    #[must_use]
    pub fn new(authz: AuthzConfig, bindings: Arc<TokenBindingStore>) -> Self {
        Self { authz, bindings }
    }

    /// Build the `Authenticated` outcome for a resolved principal record,
    /// carrying its configured `allowed_agents` as bindable aliases.
    fn authenticated(rec: &PrincipalRecord) -> AuthOutcome {
        let aliases = rec.allowed_agents.iter().cloned().map(AgentAlias).collect();
        AuthOutcome::Authenticated(
            Principal::new(
                PrincipalId::from(rec.id.clone()),
                rec.id.clone(),
                AuthMethod::Native,
            )
            .with_allowed_aliases(aliases),
        )
    }
}

#[async_trait]
impl AuthProvider for PairingAuthProvider {
    fn name(&self) -> &str {
        "pairing"
    }

    fn method(&self) -> AuthMethod {
        AuthMethod::Native
    }

    fn accepts(&self, credential: &Credential) -> bool {
        matches!(credential, Credential::Bearer(_) | Credential::Mtls { .. })
    }

    async fn verify(&self, credential: &Credential) -> AuthOutcome {
        // Derive the (optional) bearer token and (optional) mTLS device_id
        // from whichever accepted credential shape was presented. A device
        // -only credential (no token) resolves purely by device_id below.
        let (token, device_id): (Option<&str>, Option<&str>) = match credential {
            Credential::Bearer(tok) => (Some(tok.as_str()), None),
            Credential::Mtls { device_id, token } => (token.as_deref(), Some(device_id.as_str())),
            _ => {
                return AuthOutcome::Denied {
                    reason: DenyReason::NoCredential,
                };
            }
        };
        // 1. No principals configured → legacy shared-operator (unchanged).
        if !self.authz.is_enforced() {
            return AuthOutcome::Trusted(Principal::shared_operator());
        }
        let hash = token.map(PairingGuard::token_hash);
        // 2. Live runtime binding store (auto-managed by `--principal` pairing).
        //    Bindings are keyed by token hash only, so this step is skipped
        //    entirely for a device-only credential (no token presented).
        //    A binding names a principal id; it only grants access if that id
        //    still resolves in config. If the admin removed the principal, the
        //    stale binding fails closed (Denied) rather than silently allowing.
        if let Some(pid) = hash.as_deref().and_then(|h| self.bindings.get(h)) {
            return match self.authz.by_id(&pid) {
                Some(rec) => Self::authenticated(rec),
                None => AuthOutcome::Denied {
                    reason: DenyReason::BadCredential,
                },
            };
        }
        // 3. Manual config pins (`token_hashes`/`device_ids`). `hash` is `""`
        //    (never a real SHA-256 hex digest) when no token was presented, so
        //    this can only match on `device_id`.
        match self.authz.lookup(hash.as_deref().unwrap_or(""), device_id) {
            Some(rec) => Self::authenticated(rec),
            // 4. Neither bound nor pinned under enforcement → fail closed.
            None => AuthOutcome::Denied {
                reason: DenyReason::BadCredential,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    // `PrincipalRecord` and `TokenBindingStore` come in via `super::*` (the
    // module now imports them at the top for the resolution logic).
    use super::*;

    fn provider_with(alias: &str, tok: &str) -> PairingAuthProvider {
        let authz = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec![alias.into()],
                device_ids: vec![],
                token_hashes: vec![PairingGuard::token_hash(tok)],
            }],
        };
        PairingAuthProvider::new(authz, Arc::new(TokenBindingStore::new_ephemeral()))
    }

    #[tokio::test]
    async fn mapped_token_authenticates_with_allowed_aliases() {
        let p = provider_with("crm-bot", "tok-a");
        let out = p.verify(&Credential::Bearer("tok-a".into())).await;
        let principal = out.principal().expect("allowed");
        assert!(principal.may_bind("crm-bot"));
        assert!(!principal.may_bind("hr-bot"));
    }

    #[tokio::test]
    async fn unmapped_token_is_denied_when_enforced() {
        let p = provider_with("crm-bot", "tok-a");
        let out = p.verify(&Credential::Bearer("wrong".into())).await;
        assert!(out.principal().is_none()); // Denied
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::BadCredential
            }
        ));
    }

    /// Build a provider whose store binds `tok`'s hash to `bound_id`, with the
    /// given configured principals. Exercises the store resolution branch.
    fn provider_with_binding(
        principals: Vec<PrincipalRecord>,
        tok: &str,
        bound_id: &str,
    ) -> PairingAuthProvider {
        let store = TokenBindingStore::new_ephemeral();
        store
            .set(PairingGuard::token_hash(tok), bound_id.into())
            .unwrap();
        PairingAuthProvider::new(AuthzConfig { principals }, Arc::new(store))
    }

    #[tokio::test]
    async fn store_bound_token_authenticates_with_configured_principal() {
        // alice is configured (permissions), and the store binds tok-b → alice
        // although alice has NO token_hashes pin. Resolution must come from the
        // store, effective with no config-file edit.
        let alice = PrincipalRecord {
            id: "alice".into(),
            allowed_agents: vec!["crm-bot".into()],
            device_ids: vec![],
            token_hashes: vec![],
        };
        let p = provider_with_binding(vec![alice], "tok-b", "alice");
        let out = p.verify(&Credential::Bearer("tok-b".into())).await;
        let principal = out.principal().expect("store-bound token authenticates");
        assert!(principal.may_bind("crm-bot"));
        assert!(!principal.may_bind("hr-bot"));
    }

    #[tokio::test]
    async fn store_binding_to_missing_principal_is_denied() {
        // The store names "ghost", but config has only alice → fail-closed
        // (admin removed the principal; the stale binding must NOT allow).
        let alice = PrincipalRecord {
            id: "alice".into(),
            allowed_agents: vec!["crm-bot".into()],
            device_ids: vec![],
            token_hashes: vec![],
        };
        let p = provider_with_binding(vec![alice], "tok-c", "ghost");
        let out = p.verify(&Credential::Bearer("tok-c".into())).await;
        assert!(out.principal().is_none());
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::BadCredential
            }
        ));
    }

    #[tokio::test]
    async fn neither_bound_nor_pinned_is_denied_when_enforced() {
        // Enforced config, empty store, token not pinned → Denied.
        let p = provider_with("crm-bot", "tok-a");
        let out = p.verify(&Credential::Bearer("stranger".into())).await;
        assert!(out.principal().is_none());
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::BadCredential
            }
        ));
    }

    #[tokio::test]
    async fn legacy_mode_is_shared_operator() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        let out = p.verify(&Credential::Bearer("anything".into())).await;
        assert_eq!(
            out.principal().unwrap().auth_method,
            AuthMethod::SharedOperator
        );
    }

    #[tokio::test]
    async fn non_bearer_credential_is_denied() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        let out = p.verify(&Credential::None).await;
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::NoCredential
            }
        ));
    }

    #[test]
    fn accepts_bearer_and_mtls_credentials() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        assert!(p.accepts(&Credential::Bearer("x".into())));
        assert!(p.accepts(&Credential::Mtls {
            device_id: "dev-abc".into(),
            token: None,
        }));
        assert!(!p.accepts(&Credential::None));
        assert!(!p.accepts(&Credential::Peercred { uid: 0 }));
    }

    fn provider_with_device(alias: &str, device_id: &str) -> PairingAuthProvider {
        let authz = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec![alias.into()],
                device_ids: vec![device_id.into()],
                token_hashes: vec![],
            }],
        };
        PairingAuthProvider::new(authz, Arc::new(TokenBindingStore::new_ephemeral()))
    }

    #[tokio::test]
    async fn device_only_credential_matching_device_id_authenticates() {
        let p = provider_with_device("crm-bot", "dev-abc");
        let out = p
            .verify(&Credential::Mtls {
                device_id: "dev-abc".into(),
                token: None,
            })
            .await;
        let principal = out.principal().expect("matching device_id authenticates");
        assert!(principal.may_bind("crm-bot"));
    }

    #[tokio::test]
    async fn device_only_credential_with_no_matching_device_id_is_denied() {
        let p = provider_with_device("crm-bot", "dev-abc");
        let out = p
            .verify(&Credential::Mtls {
                device_id: "dev-nope".into(),
                token: None,
            })
            .await;
        assert!(out.principal().is_none());
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::BadCredential
            }
        ));
    }

    #[tokio::test]
    async fn mtls_credential_with_paired_token_still_resolves_by_token() {
        // A connection presenting BOTH a client cert and a bearer resolves via
        // the normal token path (device_id unset in config for this principal).
        let p = provider_with("crm-bot", "tok-a");
        let out = p
            .verify(&Credential::Mtls {
                device_id: "unregistered-device".into(),
                token: Some("tok-a".into()),
            })
            .await;
        let principal = out.principal().expect("token still resolves");
        assert!(principal.may_bind("crm-bot"));
    }

    #[test]
    fn name_and_method_are_native() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        assert_eq!(p.name(), "pairing");
        assert_eq!(p.method(), AuthMethod::Native);
    }
}
