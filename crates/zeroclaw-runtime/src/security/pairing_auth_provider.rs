//! [`AuthProvider`] that maps a paired bearer token to a distinct
//! [`IdentitySubject::Roster`] identity via `[[authz.principals]]` (fork-local
//! principal → bound-profile map) plus the live [`TokenBindingStore`].
//!
//! Fail-closed contract: this provider NEVER returns the shared-operator
//! sentinel. A bearer that maps to no configured principal — including when
//! no `[[authz.principals]]` exists at all — is `Denied { NotEntitled }`; an
//! empty bearer is `Denied { BadCredential }`. The legacy single-operator
//! fallback (`Principal::shared_operator` + `ResolvedGrants::all()`) lives
//! ONLY in the gateway's `/acp` callers, guarded by their own enforcement
//! predicate (any `[[authz.principals]]` OR any `[oidc.<alias>]`), so an
//! OIDC-only deployment can never reach the shared operator through this
//! provider either.
//!
//! This provider emits IDENTITY ONLY (Rev 8 contract, same as every other
//! `AuthProvider`): it never computes grants. A `Roster{principal_id}`
//! identity's actual entitlement (which `[permission_profiles.<alias>]` it
//! holds) is resolved separately by the shared `PrincipalResolver`, whose
//! roster is merged from BOTH `[users.<name>]` and `[[authz.principals]]` —
//! see `principal_resolver::ResolverPolicy::from_config`.
//!
//! Registers under the name `"pairing"` — distinct from upstream's own
//! `"native"` provider (which always resolves a paired bearer to the
//! shared-operator sentinel, never a distinct principal; see
//! `auth_provider::native`'s module doc). The two coexist: `"native"` keeps
//! its existing behavior for any caller that selects it, while a caller that
//! explicitly selects `"pairing"` (today: only `/acp`'s dedicated registry,
//! see `zeroclaw-gateway`'s `acp::build_provider_registry`) gets distinct
//! per-device/per-user principals when authz is enforced.

use std::sync::Arc;

use async_trait::async_trait;
use zeroclaw_api::principal::{
    AuthMethod, AuthOutcome, AuthenticatedIdentity, DenyReason, IdentitySubject,
};
use zeroclaw_config::authz::{AuthzConfig, TokenBindingStore};
use zeroclaw_config::pairing::PairingGuard;

use super::auth_provider::{AuthProvider, Credential};

/// Maps a paired bearer token to a distinct `Roster` identity via
/// `[[authz.principals]]` plus the live [`TokenBindingStore`]. Always
/// fail-closed: an unmapped bearer is denied, never the shared operator.
pub struct PairingAuthProvider {
    authz: AuthzConfig,
    /// Live, shared runtime binding store: `token_hash -> principal_id`
    /// written when a principal-tagged binding is set. Shared `Arc` with the
    /// gateway so a freshly-bound token resolves on the next connect with no
    /// reload.
    bindings: Arc<TokenBindingStore>,
}

impl PairingAuthProvider {
    #[must_use]
    pub fn new(authz: AuthzConfig, bindings: Arc<TokenBindingStore>) -> Self {
        Self { authz, bindings }
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
        matches!(credential, Credential::Bearer(_))
    }

    async fn verify(&self, credential: &Credential) -> AuthOutcome {
        let Credential::Bearer(token) = credential else {
            return AuthOutcome::Denied {
                reason: DenyReason::NoCredential,
            };
        };
        if token.is_empty() {
            return AuthOutcome::Denied {
                reason: DenyReason::BadCredential,
            };
        }
        let hash = PairingGuard::token_hash(token);
        // 1. Live runtime binding store (auto-managed when a principal-tagged
        //    token is bound). A binding names a principal id; it only grants
        //    identity if that id still resolves in config. If the admin
        //    removed the principal, the stale binding fails closed (Denied)
        //    rather than silently allowing.
        if let Some(pid) = self.bindings.get(&hash) {
            return match self.authz.by_id(&pid) {
                Some(rec) => AuthOutcome::Verified(AuthenticatedIdentity::new(
                    IdentitySubject::Roster {
                        principal_id: rec.id.clone(),
                    },
                    AuthMethod::Native,
                )),
                None => AuthOutcome::Denied {
                    reason: DenyReason::NotEntitled,
                },
            };
        }
        // 2. Manual config pins (`token_hashes`).
        match self.authz.lookup(&hash) {
            Some(rec) => AuthOutcome::Verified(AuthenticatedIdentity::new(
                IdentitySubject::Roster {
                    principal_id: rec.id.clone(),
                },
                AuthMethod::Native,
            )),
            // 3. Neither bound nor pinned → no principal, fail closed. This
            //    includes the zero-principals config: the shared-operator
            //    fallback is the gateway caller's decision, never this
            //    provider's.
            None => AuthOutcome::Denied {
                reason: DenyReason::NotEntitled,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::authz::PrincipalRecord;

    fn authz_with(principals: Vec<PrincipalRecord>) -> AuthzConfig {
        AuthzConfig { principals }
    }

    fn provider_with(id: &str, tok: &str, profiles: Vec<String>) -> PairingAuthProvider {
        let authz = authz_with(vec![PrincipalRecord {
            id: id.into(),
            token_hashes: vec![PairingGuard::token_hash(tok)],
            profiles,
        }]);
        PairingAuthProvider::new(authz, Arc::new(TokenBindingStore::new_ephemeral()))
    }

    #[tokio::test]
    async fn mapped_token_verifies_as_a_distinct_roster_identity() {
        let p = provider_with("alice", "tok-a", vec!["crm".into()]);
        let out = p.verify(&Credential::Bearer("tok-a".into())).await;
        let AuthOutcome::Verified(identity) = out else {
            panic!("expected Verified, got {out:?}");
        };
        assert_eq!(
            identity.subject,
            IdentitySubject::Roster {
                principal_id: "alice".to_string()
            }
        );
        assert_eq!(identity.method, AuthMethod::Native);
    }

    #[tokio::test]
    async fn unmapped_token_is_denied_when_enforced() {
        let p = provider_with("alice", "tok-a", vec!["crm".into()]);
        let out = p.verify(&Credential::Bearer("wrong".into())).await;
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::NotEntitled
            }
        ));
    }

    #[tokio::test]
    async fn empty_bearer_is_a_bad_credential() {
        let p = provider_with("alice", "tok-a", vec!["crm".into()]);
        let out = p.verify(&Credential::Bearer(String::new())).await;
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::BadCredential
            }
        ));
    }

    #[tokio::test]
    async fn store_bound_token_verifies_via_the_binding_store() {
        // alice is configured, and the store binds tok-b -> alice although
        // alice has NO token_hashes pin. Resolution must come from the store,
        // effective with no config-file edit.
        let authz = authz_with(vec![PrincipalRecord {
            id: "alice".into(),
            token_hashes: vec![],
            profiles: vec!["crm".into()],
        }]);
        let store = TokenBindingStore::new_ephemeral();
        store
            .set(PairingGuard::token_hash("tok-b"), "alice".into())
            .unwrap();
        let p = PairingAuthProvider::new(authz, Arc::new(store));
        let out = p.verify(&Credential::Bearer("tok-b".into())).await;
        let AuthOutcome::Verified(identity) = out else {
            panic!("expected Verified, got {out:?}");
        };
        assert_eq!(
            identity.subject,
            IdentitySubject::Roster {
                principal_id: "alice".to_string()
            }
        );
    }

    #[tokio::test]
    async fn store_binding_to_missing_principal_is_denied() {
        // The store names "ghost", but config has only alice -> fail-closed
        // (admin removed the principal; the stale binding must NOT allow).
        let authz = authz_with(vec![PrincipalRecord {
            id: "alice".into(),
            token_hashes: vec![],
            profiles: vec!["crm".into()],
        }]);
        let store = TokenBindingStore::new_ephemeral();
        store
            .set(PairingGuard::token_hash("tok-c"), "ghost".into())
            .unwrap();
        let p = PairingAuthProvider::new(authz, Arc::new(store));
        let out = p.verify(&Credential::Bearer("tok-c".into())).await;
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::NotEntitled
            }
        ));
    }

    /// With ZERO principals configured the provider still denies: it never
    /// returns the shared-operator sentinel. The legacy fallback is the
    /// gateway caller's decision under its own enforcement predicate.
    #[tokio::test]
    async fn zero_principals_denies_and_never_returns_shared_operator() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        let out = p.verify(&Credential::Bearer("anything".into())).await;
        assert!(
            matches!(
                out,
                AuthOutcome::Denied {
                    reason: DenyReason::NotEntitled
                }
            ),
            "got {out:?}"
        );
    }

    /// A binding alone cannot authenticate when no principal is configured
    /// either (the bound id has nothing to resolve to).
    #[tokio::test]
    async fn zero_principals_with_a_binding_still_denies() {
        let store = TokenBindingStore::new_ephemeral();
        store
            .set(PairingGuard::token_hash("tok-d"), "alice".into())
            .unwrap();
        let p = PairingAuthProvider::new(AuthzConfig::default(), Arc::new(store));
        let out = p.verify(&Credential::Bearer("tok-d".into())).await;
        assert!(matches!(out, AuthOutcome::Denied { .. }), "got {out:?}");
    }

    #[tokio::test]
    async fn non_bearer_credential_is_denied() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        let out = p.verify(&Credential::Peercred { uid: 0 }).await;
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::NoCredential
            }
        ));
    }

    #[test]
    fn accepts_bearer_only() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        assert!(p.accepts(&Credential::Bearer("x".into())));
        assert!(!p.accepts(&Credential::None));
        assert!(!p.accepts(&Credential::Peercred { uid: 0 }));
    }

    #[test]
    fn name_and_method_are_pairing_native() {
        let p = PairingAuthProvider::new(
            AuthzConfig::default(),
            Arc::new(TokenBindingStore::new_ephemeral()),
        );
        assert_eq!(p.name(), "pairing");
        assert_eq!(p.method(), AuthMethod::Native);
    }
}
