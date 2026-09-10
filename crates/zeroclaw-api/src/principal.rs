//! The authenticated subject — the single `Principal` every gateway/RPC/ACP
//! connection carries once an [`crate`] auth provider has verified a credential.

use serde::{Deserialize, Serialize};

/// Stable, opaque subject id. The audit `Actor`, the approval-routing key, the
/// provenance origin, and (A2A) the peer join key. For an OIDC user this equals
/// the IdP `sub`; for the shared-bearer / trusted-local path it is the sentinel
/// [`PrincipalId::SHARED_OPERATOR`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrincipalId(pub String);

impl PrincipalId {
    /// Sentinel id for the single-operator / trusted-local path (no distinct IdP
    /// principal). Lets callers treat "trusted, but anonymous operator" as a real
    /// `Principal` instead of branching on `Option`.
    pub const SHARED_OPERATOR: &'static str = "shared-operator";

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PrincipalId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PrincipalId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// An agent alias a principal may bind at session start. Newtype so it never gets
/// confused with an arbitrary `String` in grant checks.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentAlias(pub String);

impl AgentAlias {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AuthMethod {
    /// No authentication performed (default; an unbound connection).
    #[default]
    None,
    /// Explicitly-trusted connection with no distinct IdP principal — today's
    /// shared pairing bearer / trusted-local stdio. Carries the
    /// [`PrincipalId::SHARED_OPERATOR`] sentinel.
    SharedOperator,
    /// External OpenID Connect IdP (RFCheadline provider).
    Oidc,
    /// Challenge-response against a registered SSH public key.
    SshKey,
    /// Local Unix-socket / named-pipe peer credential (`SO_PEERCRED`).
    Peercred,
    /// The existing `PairingGuard` bearer token (continuity / operator bootstrap).
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Principal {
    pub id: PrincipalId,
    /// Human/account identifier from the identity source (e.g. OIDC `sub`).
    /// Equals `id.0` for a real user; sentinel for [`AuthMethod::SharedOperator`].
    pub user_id: String,
    /// Coarse roles the identity source asserted (drives `IamPolicy` mapping).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Fine-grained scopes/capabilities granted this session.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// How this principal authenticated.
    #[serde(default)]
    pub auth_method: AuthMethod,
    /// Whether a second factor was completed (drives any step-up policy).
    #[serde(default)]
    pub mfa_verified: bool,
    /// Session expiry, UNIX seconds; `0` = no expiry.
    #[serde(default)]
    pub expires_at: u64,
    /// Snapshot of the agent aliases this principal was granted at auth time
    /// (best-effort/audit value; see [`Principal::may_bind`]). NOT the live
    /// dispatch gate: every privileged chokepoint recomputes the caller's
    /// actual grant set fresh from `&Config` on each call
    /// ([`Principal::is_entitled_to_alias`]) so a config edit applies with no
    /// reconnect, instead of trusting this frozen field.
    #[serde(default)]
    pub allowed_aliases: Vec<AgentAlias>,
}

impl Principal {
    #[must_use]
    pub fn shared_operator() -> Self {
        Self {
            id: PrincipalId(PrincipalId::SHARED_OPERATOR.to_owned()),
            user_id: PrincipalId::SHARED_OPERATOR.to_owned(),
            roles: Vec::new(),
            scopes: Vec::new(),
            auth_method: AuthMethod::SharedOperator,
            mfa_verified: false,
            expires_at: 0,
            allowed_aliases: Vec::new(),
        }
    }

    /// Construct an authenticated principal with the given subject id and method.
    /// Grants default to empty; attach claims via the `with_*` builder setters.
    /// This is the construction path other crates (the providers) must use because
    /// the struct is `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        id: impl Into<PrincipalId>,
        user_id: impl Into<String>,
        auth_method: AuthMethod,
    ) -> Self {
        Self {
            id: id.into(),
            user_id: user_id.into(),
            roles: Vec::new(),
            scopes: Vec::new(),
            auth_method,
            mfa_verified: false,
            expires_at: 0,
            allowed_aliases: Vec::new(),
        }
    }

    /// Attach the role claims the identity source asserted.
    #[must_use]
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Attach the scope claims granted this session.
    #[must_use]
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Mark MFA as completed.
    #[must_use]
    pub fn with_mfa_verified(mut self, mfa_verified: bool) -> Self {
        self.mfa_verified = mfa_verified;
        self
    }

    /// Set the session expiry (UNIX seconds; `0` = none).
    #[must_use]
    pub fn with_expires_at(mut self, expires_at: u64) -> Self {
        self.expires_at = expires_at;
        self
    }

    /// Attach the agent aliases this principal may bind.
    #[must_use]
    pub fn with_allowed_aliases(mut self, allowed_aliases: Vec<AgentAlias>) -> Self {
        self.allowed_aliases = allowed_aliases;
        self
    }

    /// Whether the given alias is in this principal's frozen `allowed_aliases`
    /// snapshot (taken at auth time). `"*"` grants every alias. This is NOT
    /// the live dispatch gate — see [`Principal::is_entitled_to_alias`], which
    /// takes a caller-supplied live grant set instead of reading this
    /// snapshot. `may_bind` remains useful for auth-provider tests and as a
    /// best-effort/audit value on the snapshot itself.
    #[must_use]
    pub fn may_bind(&self, alias: &str) -> bool {
        self.allowed_aliases
            .iter()
            .any(|a| a.as_str() == "*" || a.as_str() == alias)
    }

    /// `true` once a *distinct* identity source authenticated this principal —
    /// i.e. not unbound ([`AuthMethod::None`]) and not the shared-operator
    /// sentinel. A2A distinct-principal routing keys on this.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        !matches!(
            self.auth_method,
            AuthMethod::None | AuthMethod::SharedOperator
        )
    }

    /// The fork-local entitlement predicate: `true` if this principal may bind
    /// or dispatch `alias`, given `live_allowed` — the caller's LIVE grant set.
    /// Callers must recompute `live_allowed` from `&Config`
    /// (`AuthzConfig::effective_agents`) on EVERY privileged op, never read it
    /// once and cache it: this is what lets an `[[authz.profiles]]` /
    /// `authz.principals` edit take effect on an already-open connection with
    /// no daemon restart. Do NOT pass `self.allowed_aliases` here — that is
    /// the frozen snapshot taken at auth time and is no longer consulted by
    /// this gate.
    ///
    /// The trusted-local / legacy [`Principal::shared_operator`]
    /// (`!is_authenticated`) may reach any configured alias regardless of
    /// `live_allowed`, preserving today's behaviour when no authz principals
    /// are configured; a *distinct* authenticated principal is limited to
    /// `live_allowed` (explicit alias or `"*"`). This is THE shared gate for
    /// every dispatch chokepoint (ACP bind today; REST/A2A once they resolve
    /// an authenticated principal).
    #[must_use]
    pub fn is_entitled_to_alias(&self, alias: &str, live_allowed: &[String]) -> bool {
        !self.is_authenticated() || live_allowed.iter().any(|a| a == "*" || a == alias)
    }
}

/// Filter agent aliases down to the ones `principal` may see on a private
/// list surface — RPC `agents/list` (zerocode) and REST
/// `GET /api/config/agent-options` (dashboard). Applies the same gate
/// predicate as the ACP bind-time check
/// (`crates/zeroclaw-channels/src/orchestrator/acp_server.rs`): an
/// authenticated principal is shown only the aliases in `live_allowed`
/// (recomputed by the caller from live `&Config`, never cached); an
/// unauthenticated/trusted principal — today's [`Principal::shared_operator`]
/// on both of these surfaces, since neither resolves an authenticated
/// principal yet — sees every alias regardless of `live_allowed`. Do not
/// substitute a bare `live_allowed.contains(...)` check: `shared_operator`
/// must short-circuit on `!is_authenticated()` or the trusted-local path
/// would see an empty list.
#[must_use]
pub fn filter_agents_for_principal<'a>(
    all: &'a [&'a str],
    principal: &Principal,
    live_allowed: &[String],
) -> Vec<&'a str> {
    all.iter()
        .copied()
        .filter(|alias| principal.is_entitled_to_alias(alias, live_allowed))
        .collect()
}

/// Why a credential was rejected. Fail-closed: any ambiguity ⇒ a `Denied` variant,
/// never a silent allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyReason {
    /// No credential was presented.
    NoCredential,
    /// A credential was presented but failed verification.
    BadCredential,
    /// The credential/session has expired.
    TokenExpired,
    /// A second factor is required and was not satisfied.
    MfaRequired,
    /// The principal is not entitled to the requested agent alias.
    AliasNotEntitled,
    /// The provider/config is misconfigured (fail closed, do not allow).
    Misconfigured,
}

/// The single result every auth surface returns. Misroute/timeout/malformed ⇒
/// [`AuthOutcome::Denied`], NEVER a silent allow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthOutcome {
    /// A distinct identity source authenticated the caller.
    Authenticated(Principal),
    /// An explicitly-trusted connection with no distinct IdP principal — carries
    /// the [`Principal::shared_operator`] sentinel so callers never branch on
    /// `Option`.
    Trusted(Principal),
    /// The credential was rejected.
    Denied { reason: DenyReason },
}

impl AuthOutcome {
    /// The bound principal if the outcome allows the connection (authenticated or
    /// trusted), else `None`.
    #[must_use]
    pub fn principal(&self) -> Option<&Principal> {
        match self {
            Self::Authenticated(p) | Self::Trusted(p) => Some(p),
            Self::Denied { .. } => None,
        }
    }

    /// Whether the connection is allowed to proceed at all (still subject to
    /// per-method grant checks downstream).
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Authenticated(_) | Self::Trusted(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_operator_is_trusted_but_not_authenticated() {
        let p = Principal::shared_operator();
        assert_eq!(p.id.as_str(), PrincipalId::SHARED_OPERATOR);
        assert_eq!(p.auth_method, AuthMethod::SharedOperator);
        assert!(!p.is_authenticated());
    }

    #[test]
    fn a_real_principal_is_authenticated() {
        let p = Principal {
            id: PrincipalId::from("alice"),
            user_id: "alice".to_owned(),
            roles: vec!["operator".to_owned()],
            scopes: vec![],
            auth_method: AuthMethod::Oidc,
            mfa_verified: true,
            expires_at: 0,
            allowed_aliases: vec![AgentAlias("main".to_owned())],
        };
        assert!(p.is_authenticated());
    }

    #[test]
    fn is_entitled_to_alias_gate() {
        // Trusted-local / legacy: reaches any alias regardless of
        // `live_allowed` — even an empty one.
        let shared = Principal::shared_operator();
        assert!(shared.is_entitled_to_alias("anything", &[]));
        assert!(shared.is_entitled_to_alias("crm-bot", &[]));

        // Distinct authenticated principal: limited to the live grant set.
        let alice = Principal {
            id: PrincipalId::from("alice"),
            user_id: "alice".to_owned(),
            roles: vec![],
            scopes: vec![],
            auth_method: AuthMethod::Oidc,
            mfa_verified: false,
            expires_at: 0,
            allowed_aliases: vec![AgentAlias("crm-bot".to_owned())],
        };
        assert!(alice.is_entitled_to_alias("crm-bot", &["crm-bot".to_owned()]));
        assert!(!alice.is_entitled_to_alias("payroll-bot", &["crm-bot".to_owned()]));

        // Empty live set denies even an explicit configured alias — the
        // frozen `allowed_aliases` snapshot above is not consulted.
        assert!(!alice.is_entitled_to_alias("crm-bot", &[]));

        // Wildcard in the live set grants every alias.
        assert!(alice.is_entitled_to_alias("payroll-bot", &["*".to_owned()]));
    }

    #[test]
    fn is_entitled_to_alias_ignores_frozen_snapshot() {
        // The frozen `allowed_aliases` snapshot (taken at auth time) must
        // never be consulted — only the caller-supplied live set decides.
        let alice = Principal::new(PrincipalId::from("alice"), "alice", AuthMethod::Oidc)
            .with_allowed_aliases(vec![AgentAlias("stale-bot".into())]);
        assert!(!alice.is_entitled_to_alias("stale-bot", &[]));
        assert!(alice.is_entitled_to_alias("fresh-bot", &["fresh-bot".to_owned()]));
    }

    #[test]
    fn auth_outcome_allow_and_principal_accessors() {
        let ok = AuthOutcome::Trusted(Principal::shared_operator());
        assert!(ok.is_allowed());
        assert!(ok.principal().is_some());

        let no = AuthOutcome::Denied {
            reason: DenyReason::NoCredential,
        };
        assert!(!no.is_allowed());
        assert!(no.principal().is_none());
    }

    #[test]
    fn principal_roundtrips_through_json() {
        let p = Principal::shared_operator();
        let s = serde_json::to_string(&p).expect("serialize");
        let back: Principal = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(p, back);
    }

    #[test]
    fn auth_method_serializes_snake_case() {
        let j = serde_json::to_string(&AuthMethod::SshKey).expect("serialize");
        assert_eq!(j, "\"ssh_key\"");
    }

    #[test]
    fn authenticated_outcome_is_allowed_and_exposes_principal() {
        // Existing tests cover Trusted and Denied but not the Authenticated arm.
        let outcome = AuthOutcome::Authenticated(Principal::shared_operator());
        assert!(outcome.is_allowed());
        assert!(outcome.principal().is_some());
    }

    #[test]
    fn none_auth_method_is_not_authenticated() {
        let mut p = Principal::shared_operator();
        p.auth_method = AuthMethod::None;
        assert!(!p.is_authenticated());
    }

    #[test]
    fn builder_methods_set_fields() {
        let p = Principal::shared_operator()
            .with_roles(vec!["admin".to_owned()])
            .with_scopes(vec!["read".to_owned(), "write".to_owned()])
            .with_mfa_verified(true)
            .with_expires_at(42)
            .with_allowed_aliases(vec![AgentAlias("bot".to_owned())]);
        assert_eq!(p.roles, vec!["admin".to_owned()]);
        assert_eq!(p.scopes.len(), 2);
        assert!(p.mfa_verified);
        assert_eq!(p.expires_at, 42);
        assert_eq!(p.allowed_aliases.len(), 1);
        assert_eq!(p.allowed_aliases[0].as_str(), "bot");
    }

    #[test]
    fn may_bind_explicit_and_wildcard() {
        let p = Principal::new(PrincipalId::from("alice"), "alice", AuthMethod::Native)
            .with_allowed_aliases(vec![AgentAlias("crm-bot".into())]);
        assert!(p.may_bind("crm-bot"));
        assert!(!p.may_bind("hr-bot"));

        let admin = Principal::new(PrincipalId::from("admin"), "admin", AuthMethod::Native)
            .with_allowed_aliases(vec![AgentAlias("*".into())]);
        assert!(admin.may_bind("anything"));
    }

    #[test]
    fn agents_list_filters_by_principal() {
        let principal = Principal::new(PrincipalId::from("alice"), "alice", AuthMethod::Native);
        let listed = filter_agents_for_principal(
            &["crm-bot", "hr-bot"],
            &principal,
            &["crm-bot".to_owned()],
        );
        assert_eq!(listed, vec!["crm-bot"]);
    }

    #[test]
    fn agents_list_is_empty_for_a_role_with_no_agents() {
        // A bound-but-empty role (no profiles, or profiles with no
        // allowed_agents) must yield nothing — deny-by-default, not a
        // silent fallback to "show everything".
        let principal = Principal::new(PrincipalId::from("alice"), "alice", AuthMethod::Native);
        let listed = filter_agents_for_principal(&["crm-bot", "hr-bot"], &principal, &[]);
        assert!(listed.is_empty());
    }

    #[test]
    fn agents_list_shows_everything_for_wildcard_live_allowed() {
        // An admin-bound principal's live set is `["*"]` — every alias.
        let principal = Principal::new(PrincipalId::from("alice"), "alice", AuthMethod::Native);
        let listed =
            filter_agents_for_principal(&["crm-bot", "hr-bot"], &principal, &["*".to_owned()]);
        assert_eq!(listed, vec!["crm-bot", "hr-bot"]);
    }

    #[test]
    fn agents_list_shows_everything_for_shared_operator() {
        // The trusted-local / shared-bearer path carries no `allowed_aliases`.
        // The gate must fall back to "show everything" here, not filter to
        // empty — this bug has bitten this feature twice before.
        let principal = Principal::shared_operator();
        let listed = filter_agents_for_principal(&["crm-bot", "hr-bot"], &principal, &[]);
        assert_eq!(listed, vec!["crm-bot", "hr-bot"]);
    }

    #[test]
    fn every_deny_reason_is_not_allowed() {
        for reason in [
            DenyReason::NoCredential,
            DenyReason::BadCredential,
            DenyReason::TokenExpired,
            DenyReason::MfaRequired,
            DenyReason::AliasNotEntitled,
            DenyReason::Misconfigured,
        ] {
            let outcome = AuthOutcome::Denied { reason };
            assert!(!outcome.is_allowed());
            assert!(outcome.principal().is_none());
        }
    }
}
