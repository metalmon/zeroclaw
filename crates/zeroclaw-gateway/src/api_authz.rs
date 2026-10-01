//! Fork-local authz admin control plane: the `GET /api/authz/principals`
//! listing the panel's principal picker (role-at-pairing) reads, plus the
//! shared admin gate it is wired through.
//!
//! Schema reality on this branch: bearer-paired principals live in
//! `[[authz.principals]]` (`zeroclaw_config::authz::PrincipalRecord`), their
//! grants are the upstream `[permission_profiles.<alias>]` entries those
//! records name in `profiles`. The admin bit is therefore DERIVED: a
//! principal is admin iff any profile it is bound to has `admin = true`
//! (`PermissionProfileConfig::admin`).
//!
//! Security-critical: this surface enumerates who is bound to what, so it is
//! never served to merely-paired devices once authz is enforced —
//! `require_admin` is the chokepoint. The whole module lives on the PRIVATE
//! listener only (see `public_router` in `lib.rs`), which is what makes the
//! unconfigured-authz fallback below tolerable.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::AppState;
use super::api::require_auth;

// ── Admin gate ──────────────────────────────────────────────────────

/// Resolve the caller's principal for an admin-gated REST route — the same
/// way `acp::resolve_principal` resolves an `/acp` connection's bearer — and
/// require its resolved grants to carry `admin`.
///
/// Lockout fallback (CRITICAL): while authz is UNCONFIGURED (no
/// `[[authz.principals]]` exists, i.e. `!authz.is_enforced()`), this
/// delegates entirely to [`require_auth`] (the existing paired-guard)
/// instead of demanding an admin principal. A fresh install has no
/// principals yet, so requiring admin unconditionally would lock the
/// operator out of their own control plane. The instant any principal is
/// configured, this fallback stops applying: an unresolved credential, a
/// credential the auth provider denies, or a credential that resolves to a
/// principal not bound to any `admin` profile are all a flat 403.
///
/// Safe only because the two-listener split keeps this whole control plane
/// off the public listener entirely: while unconfigured, ANY paired bearer
/// satisfies `require_auth`, which is only tolerable because only the
/// operator can reach this endpoint at all (private listener) before authz
/// is configured to restrict who "paired" even means.
///
/// The error type mirrors [`require_auth`]'s own
/// `Result<(), (StatusCode, Json<serde_json::Value>)>` shape rather than a
/// full `Response` (`clippy::result_large_err`).
pub(crate) async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if !state.config.read().authz.is_enforced() {
        return require_auth(state, headers);
    }

    let token = super::api::extract_bearer_token(headers);
    // `resolve_principal` builds its provider registry from LIVE config on
    // every call (no frozen snapshot), so a profile edit or a freshly bound
    // `--principal` token is visible here without a reload.
    let is_admin = crate::acp::resolve_principal(state, token)
        .await
        .is_some_and(|(_, grants)| grants.admin);

    if is_admin {
        return Ok(());
    }

    Err(forbidden_error())
}

fn forbidden_error() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({
            "code": "forbidden",
            "error": "this action requires a principal bound to an admin profile",
        })),
    )
}

/// Derived admin bit for a configured principal id: true when any
/// `[permission_profiles.<alias>]` it is bound to has `admin = true`.
/// Unknown profile names grant nothing (deny-by-default, same as the
/// resolver's roster merge).
pub(crate) fn principal_is_admin(config: &zeroclaw_config::schema::Config, principal_id: &str) -> bool {
    config
        .authz
        .by_id(principal_id)
        .is_some_and(|record| {
            record.profiles.iter().any(|profile| {
                config
                    .permission_profiles
                    .get(profile.trim())
                    .is_some_and(|p| p.admin)
            })
        })
}

// ── Principals listing ──────────────────────────────────────────────

/// Wire shape of one principal in `GET /api/authz/principals`. Matches the
/// panel's `PrincipalSummary` (`{id, profiles, admin, device_ids}`).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct PrincipalDto {
    pub id: String,
    pub profiles: Vec<String>,
    /// Derived: true when any profile this principal is bound to has
    /// `admin = true` (see [`principal_is_admin`]).
    pub admin: bool,
    /// Device identities bound to this principal. No mTLS/device identity is
    /// threaded through this branch's principal records, so this is always
    /// empty today; kept on the wire so the panel's shape stays stable.
    pub device_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct PrincipalsListResponse {
    pub principals: Vec<PrincipalDto>,
}

/// `GET /api/authz/principals` — list every configured
/// `[[authz.principals]]` entry (id, bound profiles, derived admin bit,
/// device ids). ADMIN-only: this enumerates who is bound to what — the
/// panel's principal picker for role-at-pairing needs it, but it is not
/// itself safe to hand to every paired device. Never returns
/// `token_hashes`.
pub async fn handle_list_principals(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    let cfg = state.config.read().clone();
    let principals = cfg
        .authz
        .principals
        .iter()
        .map(|p| PrincipalDto {
            id: p.id.clone(),
            profiles: p.profiles.clone(),
            admin: principal_is_admin(&cfg, &p.id),
            device_ids: Vec::new(),
        })
        .collect();
    Json(PrincipalsListResponse { principals }).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, header};
    use http_body_util::BodyExt;
    use zeroclaw_config::authz::{AuthzConfig, PrincipalRecord};
    use zeroclaw_config::schema::PermissionProfileConfig;
    use zeroclaw_runtime::security::pairing::PairingGuard;

    fn bearer_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers
    }

    async fn response_json(response: Response) -> (StatusCode, serde_json::Value) {
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, json)
    }

    /// Enforced authz: `alice` (admin profile) + `bob` (non-admin profile),
    /// both tokens paired in the guard and pinned by hash in config.
    fn enforced_state(tmp: &tempfile::TempDir) -> (AppState, &'static str, &'static str) {
        let alice_token = "alice-tok";
        let bob_token = "bob-tok";
        let state = crate::tests::admin_paircode_state(tmp, true, false);
        {
            let mut cfg = state.config.write();
            cfg.permission_profiles.insert(
                "ops".to_string(),
                PermissionProfileConfig {
                    admin: true,
                    ..PermissionProfileConfig::default()
                },
            );
            cfg.permission_profiles.insert(
                "viewer".to_string(),
                PermissionProfileConfig {
                    admin: false,
                    allowed_agents: vec!["*".to_string()],
                    ..PermissionProfileConfig::default()
                },
            );
            cfg.authz = AuthzConfig {
                principals: vec![
                    PrincipalRecord {
                        id: "alice".to_string(),
                        token_hashes: vec![PairingGuard::token_hash(alice_token)],
                        profiles: vec!["ops".to_string()],
                    },
                    PrincipalRecord {
                        id: "bob".to_string(),
                        token_hashes: vec![PairingGuard::token_hash(bob_token)],
                        profiles: vec!["viewer".to_string()],
                    },
                ],
            };
        }
        let paired = std::sync::Arc::new(PairingGuard::new(
            true,
            &[alice_token.to_string(), bob_token.to_string()],
            zeroclaw_config::pairing::PairingCodePolicy::default(),
        ));
        let state = AppState {
            pairing: paired,
            ..state
        };
        (state, alice_token, bob_token)
    }

    #[test]
    fn derived_admin_bit_follows_bound_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, _) = enforced_state(&tmp);
        let cfg = state.config.read().clone();
        assert!(principal_is_admin(&cfg, "alice"));
        assert!(!principal_is_admin(&cfg, "bob"));
        assert!(!principal_is_admin(&cfg, "nobody"));
    }

    #[tokio::test]
    async fn unconfigured_authz_falls_back_to_paired_guard() {
        let tmp = tempfile::tempdir().unwrap();
        // require_pairing=false → `require_auth` is open; no principals →
        // unenforced → the admin gate delegates to it.
        let state = crate::tests::admin_paircode_state(&tmp, false, false);
        assert!(require_admin(&state, &HeaderMap::new()).await.is_ok());
    }

    #[tokio::test]
    async fn configured_authz_denies_a_non_admin_principal() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, bob) = enforced_state(&tmp);
        let err = require_admin(&state, &bearer_headers(bob))
            .await
            .expect_err("bob is bound to a non-admin profile");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn configured_authz_denies_an_unknown_credential() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, _) = enforced_state(&tmp);
        let err = require_admin(&state, &bearer_headers("never-paired"))
            .await
            .expect_err("an unresolved bearer must be refused once authz is enforced");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_principals_returns_panel_shape_for_an_admin() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let response =
            handle_list_principals(State(state), bearer_headers(alice)).await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let principals = json["principals"].as_array().expect("principals array");
        assert_eq!(principals.len(), 2);
        let alice_row = principals
            .iter()
            .find(|p| p["id"] == "alice")
            .expect("alice listed");
        assert_eq!(alice_row["profiles"], serde_json::json!(["ops"]));
        assert_eq!(alice_row["admin"], true);
        assert_eq!(alice_row["device_ids"], serde_json::json!([]));
        let bob_row = principals
            .iter()
            .find(|p| p["id"] == "bob")
            .expect("bob listed");
        assert_eq!(bob_row["admin"], false);
        assert!(
            !json.to_string().contains("token_hashes"),
            "token hashes must never be enumerated"
        );
    }

    #[tokio::test]
    async fn list_principals_is_forbidden_for_a_non_admin() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, bob) = enforced_state(&tmp);
        let response = handle_list_principals(State(state), bearer_headers(bob)).await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "forbidden");
    }
}
