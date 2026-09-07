//! [`AuthProvider`] that maps a paired bearer token to a [`Principal`] via
//! `[[authz.principals]]` (fork-local principal → allowed-agents map).
//!
//! Fail-closed contract: once any principal is configured (`authz.is_enforced()`),
//! an unmapped bearer is `Denied { BadCredential }` — never a silent allow. When no
//! principal is configured (legacy/default config), any non-empty bearer is
//! `Trusted` as the [`Principal::shared_operator`] sentinel, preserving today's
//! single-operator behaviour unchanged.

use async_trait::async_trait;
use zeroclaw_api::principal::{
    AgentAlias, AuthMethod, AuthOutcome, DenyReason, Principal, PrincipalId,
};
use zeroclaw_config::authz::AuthzConfig;
use zeroclaw_config::pairing::PairingGuard;

use super::auth_provider::{AuthProvider, Credential};

/// Maps a paired bearer token to a `Principal` via `[[authz.principals]]`.
/// Fail-closed when authz is enforced; legacy shared-operator otherwise.
pub struct PairingAuthProvider {
    authz: AuthzConfig,
}

impl PairingAuthProvider {
    #[must_use]
    pub fn new(authz: AuthzConfig) -> Self {
        Self { authz }
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
        let Credential::Bearer(tok) = credential else {
            return AuthOutcome::Denied {
                reason: DenyReason::NoCredential,
            };
        };
        if !self.authz.is_enforced() {
            return AuthOutcome::Trusted(Principal::shared_operator());
        }
        let hash = PairingGuard::token_hash(tok);
        match self.authz.lookup(&hash, None) {
            Some(rec) => {
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
            None => AuthOutcome::Denied {
                reason: DenyReason::BadCredential,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::authz::PrincipalRecord;

    fn provider_with(alias: &str, tok: &str) -> PairingAuthProvider {
        let authz = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec![alias.into()],
                device_ids: vec![],
                token_hashes: vec![PairingGuard::token_hash(tok)],
            }],
        };
        PairingAuthProvider::new(authz)
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

    #[tokio::test]
    async fn legacy_mode_is_shared_operator() {
        let p = PairingAuthProvider::new(AuthzConfig::default());
        let out = p.verify(&Credential::Bearer("anything".into())).await;
        assert_eq!(
            out.principal().unwrap().auth_method,
            AuthMethod::SharedOperator
        );
    }

    #[tokio::test]
    async fn non_bearer_credential_is_denied() {
        let p = PairingAuthProvider::new(AuthzConfig::default());
        let out = p.verify(&Credential::None).await;
        assert!(matches!(
            out,
            AuthOutcome::Denied {
                reason: DenyReason::NoCredential
            }
        ));
    }

    #[test]
    fn accepts_only_bearer_credentials() {
        let p = PairingAuthProvider::new(AuthzConfig::default());
        assert!(p.accepts(&Credential::Bearer("x".into())));
        assert!(!p.accepts(&Credential::None));
        assert!(!p.accepts(&Credential::Peercred { uid: 0 }));
    }

    #[test]
    fn name_and_method_are_native() {
        let p = PairingAuthProvider::new(AuthzConfig::default());
        assert_eq!(p.name(), "pairing");
        assert_eq!(p.method(), AuthMethod::Native);
    }
}
