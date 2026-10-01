//! Fork-local authz admin control plane: the `GET /api/authz/principals`
//! listing the panel's principal picker (role-at-pairing) reads, the
//! `/api/authz/profiles` CRUD and `/api/authz/principals/{id}/profiles`
//! bind/unbind the panel's Roles page drives, principal create/delete
//! (`POST /api/authz/principals`, `DELETE /api/authz/principals/{id}`), plus
//! the shared admin gate they are all wired through.
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
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zeroclaw_api::grants::Verb;
use zeroclaw_config::api_error::{ConfigApiCode, ConfigApiError};
use zeroclaw_config::schema::{Config, PermissionProfileConfig};

use super::api::require_auth;
use super::api_authz_external::ExternalSubjectRecord;
use super::api_config::persist_and_swap;
use super::{AppState, ConfigWriteGuard};
use crate::principal_gate::{ConfigWriteSet, RequestPrincipal, authorize_config_write};

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
    let is_admin = matches!(
        crate::acp::resolve_principal(state, token).await,
        crate::acp::Resolution::Resolved(principal_and_grants) if principal_and_grants.1.admin
    );

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
pub(crate) fn principal_is_admin(
    config: &zeroclaw_config::schema::Config,
    principal_id: &str,
) -> bool {
    config.authz.by_id(principal_id).is_some_and(|record| {
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

// ── External (SSO) subjects ─────────────────────────────────────────

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ExternalSubjectsListResponse {
    pub subjects: Vec<ExternalSubjectRecord>,
}

/// `GET /api/authz/external`: every external (identity-provider) subject
/// the gateway has admitted at least once, most recently seen first. ADMIN
/// only, like the principals listing. Read-only: these subjects' grants come
/// from the provider's groups via `[oidc.<alias>].profile_map`.
pub async fn handle_list_external_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    Json(ExternalSubjectsListResponse {
        subjects: state.external_subjects.list(),
    })
    .into_response()
}

/// `DELETE /api/authz/external/{id}`: forget one seen subject (the id is
/// the canonical principal id, URL-encoded by the caller). The next login
/// records it again; nothing about its access changes. 404 when unknown.
pub async fn handle_forget_external_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    if !state.external_subjects.forget(&id) {
        return not_found(
            format!("External subject '{id}' has not been seen"),
            format!("authz.external.{id}"),
        );
    }
    Json(serde_json::json!({ "id": id, "deleted": true })).into_response()
}

// ── Shared error + persistence plumbing ─────────────────────────────

fn error_response(err: ConfigApiError) -> Response {
    let status =
        StatusCode::from_u16(err.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(err)).into_response()
}

/// 409 in the panel's `{code, message, path}` envelope. `ConfigApiCode` has
/// no conflict variant and this module must not widen the config crate, so
/// the body is assembled here.
fn conflict_response(message: String, path: String) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({
            "code": "conflict",
            "message": message,
            "path": path,
        })),
    )
        .into_response()
}

fn not_found(message: String, path: String) -> Response {
    error_response(ConfigApiError::new(ConfigApiCode::PathNotFound, message).with_path(path))
}

/// Profile ids are TOML table keys and dotted-path segments, so they are
/// kept to `[A-Za-z0-9][A-Za-z0-9_-]*`.
fn validate_profile_id(id: &str) -> Result<(), ConfigApiError> {
    let mut chars = id.chars();
    let head_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let tail_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if head_ok && tail_ok {
        return Ok(());
    }
    Err(ConfigApiError::new(
        ConfigApiCode::InvalidFormat,
        format!(
            "profile id `{id}` is invalid: use letters, digits, `_` or `-`, starting with a letter or digit"
        ),
    )
    .with_path("permission_profiles"))
}

/// `allowed_agents` entries must name a configured `[agents.<alias>]` or be
/// the explicit `"*"` wildcard. Checked up front so the panel gets a 400
/// pointing at the offending entry instead of a whole-config save failure.
fn validate_allowed_agents(
    config: &Config,
    id: &str,
    agents: &[String],
) -> Result<(), ConfigApiError> {
    for agent in agents {
        if agent == "*" || config.agents.contains_key(agent) {
            continue;
        }
        return Err(ConfigApiError::new(
            ConfigApiCode::DanglingReference,
            format!("allowed_agents names {agent:?} but [agents.{agent}] is not configured (use \"*\" for every agent)"),
        )
        .with_path(format!("permission_profiles.{id}.allowed_agents")));
    }
    Ok(())
}

/// Write `value` to `path` through the dotted-path engine so `mark_dirty`
/// and the incremental TOML writer see the edit like any other prop write.
fn set_prop(working: &mut Config, path: &str, value: &str) -> Result<(), ConfigApiError> {
    working
        .set_prop_persistent(path, value)
        .map_err(|e| ConfigApiError::from_validation(e).with_path(path))
}

fn json_list(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string())
}

/// Authorize the dirty write set for the admitted principal, then save and
/// hot-swap. `before` is the live config the working copy was cloned from.
#[allow(clippy::result_large_err)]
async fn commit(
    state: &AppState,
    principal: &RequestPrincipal,
    before: &Config,
    working: Config,
    guard: ConfigWriteGuard,
    writes: Option<(String, Verb)>,
) -> Result<(), Response> {
    let mut set = ConfigWriteSet::by_effect(
        before,
        &working,
        working.dirty_paths.iter().map(String::as_str),
    );
    if let Some((path, verb)) = writes {
        set = set.with(path, verb);
    }
    let authorization =
        authorize_config_write(principal, set, &guard).map_err(IntoResponse::into_response)?;
    persist_and_swap(state, authorization, working, guard)
        .await
        .map(|_| ())
}

// ── Profiles CRUD ───────────────────────────────────────────────────

/// Wire shape of one `[permission_profiles.<id>]` entry. Matches the panel's
/// `AuthzProfile` (`{id, allowed_agents, admin}`); the profile's other grant
/// fields (`allowed_tools`, `config_write_paths`, `grants`) are not on the
/// panel contract and are left untouched by writes through this surface.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfileDto {
    pub id: String,
    pub allowed_agents: Vec<String>,
    pub admin: bool,
}

impl ProfileDto {
    fn from_config(id: &str, profile: &PermissionProfileConfig) -> Self {
        Self {
            id: id.to_string(),
            allowed_agents: profile.allowed_agents.clone(),
            admin: profile.admin,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfilesListResponse {
    pub profiles: Vec<ProfileDto>,
}

/// `GET /api/authz/profiles` lists every configured permission profile, sorted
/// by id. Read-only, so it is gated by the paired-guard rather than the
/// admin gate: profile shape carries no credentials and no who-is-bound-to-
/// what information.
pub async fn handle_list_profiles(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    let cfg = state.config.read();
    let mut profiles: Vec<ProfileDto> = cfg
        .permission_profiles
        .iter()
        .map(|(id, p)| ProfileDto::from_config(id, p))
        .collect();
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    Json(ProfilesListResponse { profiles }).into_response()
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfileBody {
    pub id: String,
    #[serde(default)]
    pub allowed_agents: Vec<String>,
    #[serde(default)]
    pub admin: bool,
}

/// Create-or-update the `[permission_profiles.<id>]` row from `body`. When
/// `must_create` is set an existing id is a 409 (POST semantics); otherwise
/// the row is upserted (PUT semantics).
async fn upsert_profile(
    state: AppState,
    headers: HeaderMap,
    principal: RequestPrincipal,
    body: ProfileBody,
    must_create: bool,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    if let Err(e) = validate_profile_id(&body.id) {
        return error_response(e);
    }

    let guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let before = state.config.read().clone();
    let mut working = before.clone();
    let path = format!("permission_profiles.{}", body.id);
    if must_create && working.permission_profiles.contains_key(&body.id) {
        return conflict_response(
            format!("profile `{}` already exists; use PUT to update it", body.id),
            path,
        );
    }
    if let Err(e) = validate_allowed_agents(&working, &body.id, &body.allowed_agents) {
        return error_response(e);
    }
    if let Err(msg) = working.create_map_key("permission_profiles", &body.id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::InternalError, msg).with_path(&path),
        );
    }
    if let Err(e) = set_prop(
        &mut working,
        &format!("{path}.allowed_agents"),
        &json_list(&body.allowed_agents),
    ) {
        return error_response(e);
    }
    if let Err(e) = set_prop(
        &mut working,
        &format!("{path}.admin"),
        if body.admin { "true" } else { "false" },
    ) {
        return error_response(e);
    }
    if let Err(response) = commit(&state, &principal, &before, working, guard, None).await {
        return response;
    }
    Json(ProfileDto {
        id: body.id,
        allowed_agents: body.allowed_agents,
        admin: body.admin,
    })
    .into_response()
}

/// `POST /api/authz/profiles` creates a permission profile. 409 when the
/// id is already taken.
pub async fn handle_create_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Json(body): Json<ProfileBody>,
) -> Response {
    upsert_profile(state, headers, principal, body, true).await
}

/// `PUT /api/authz/profiles` is an idempotent create-or-update keyed by
/// `body.id`.
pub async fn handle_update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Json(body): Json<ProfileBody>,
) -> Response {
    upsert_profile(state, headers, principal, body, false).await
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfileIdQuery {
    pub id: String,
}

/// Matches the panel's `DeleteAuthzProfileResponse`. `affected_principals`
/// is always empty on this branch: a profile still bound to a principal is
/// refused with 409 (config validation rejects dangling profile references),
/// so the field only exists to keep the wire shape the shipped panel parses.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct DeleteProfileResponse {
    pub id: String,
    pub deleted: bool,
    pub affected_principals: Vec<String>,
}

/// `DELETE /api/authz/profiles?id=<id>` removes a permission profile. 404
/// when unknown; 409 while any `[[authz.principals]]` row (or `[users]`
/// entry) still names it, with the referencing ids in the message.
pub async fn handle_delete_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Query(q): Query<ProfileIdQuery>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }

    let guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let before = state.config.read().clone();
    let mut working = before.clone();
    let path = format!("permission_profiles.{}", q.id);
    if !working.permission_profiles.contains_key(&q.id) {
        return not_found(format!("profile `{}` is not configured", q.id), path);
    }
    let bound_principals: Vec<&str> = working
        .authz
        .principals
        .iter()
        .filter(|p| p.profiles.iter().any(|pid| pid.trim() == q.id))
        .map(|p| p.id.as_str())
        .chain(
            working
                .users
                .iter()
                .filter(|(_, u)| u.permission_profiles.iter().any(|pid| pid.trim() == q.id))
                .map(|(name, _)| name.as_str()),
        )
        .collect();
    if !bound_principals.is_empty() {
        return conflict_response(
            format!(
                "profile `{}` is still bound to {}; unbind it first",
                q.id,
                bound_principals.join(", ")
            ),
            path,
        );
    }

    if let Err(msg) = working.delete_map_key("permission_profiles", &q.id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::PathNotFound, msg).with_path(&path),
        );
    }
    working.mark_dirty(&path);
    if let Err(response) = commit(
        &state,
        &principal,
        &before,
        working,
        guard,
        Some((path, Verb::Delete)),
    )
    .await
    {
        return response;
    }
    Json(DeleteProfileResponse {
        id: q.id,
        deleted: true,
        affected_principals: Vec::new(),
    })
    .into_response()
}

// ── Principal <-> profile binding ───────────────────────────────────

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct BindProfileBody {
    pub profile_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct UnbindProfileQuery {
    pub profile_id: String,
}

/// Matches the panel's `AuthzPrincipalProfilesResponse`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct PrincipalProfilesResponse {
    pub principal_id: String,
    pub profiles: Vec<String>,
}

/// Replace `principal_id`'s `profiles` list with `profiles(current)` and
/// persist. 404 when the principal is not configured, or when
/// `required_profile` names a profile that is not. Both checks run under the
/// config write lock. Principals are created by `handle_create_principal`
/// or by pairing, never here.
#[allow(clippy::result_large_err)]
async fn write_principal_profiles(
    state: &AppState,
    principal: &RequestPrincipal,
    principal_id: &str,
    required_profile: Option<&str>,
    profiles: impl FnOnce(Vec<String>) -> Vec<String>,
) -> Result<Vec<String>, Response> {
    let guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let before = state.config.read().clone();
    let mut working = before.clone();
    let Some(current) = working
        .authz
        .by_id(principal_id)
        .map(|p| p.profiles.clone())
    else {
        return Err(not_found(
            format!("principal `{principal_id}` is not configured"),
            "authz.principals".to_string(),
        ));
    };
    if let Some(profile_id) =
        required_profile.filter(|p| !working.permission_profiles.contains_key(*p))
    {
        return Err(not_found(
            format!("profile `{profile_id}` is not configured"),
            format!("permission_profiles.{profile_id}"),
        ));
    }
    let next = profiles(current.clone());
    if next == current {
        return Ok(current);
    }
    let path = format!("authz.principals.{principal_id}.profiles");
    set_prop(&mut working, &path, &json_list(&next)).map_err(error_response)?;
    commit(state, principal, &before, working, guard, None).await?;
    Ok(next)
}

/// `PUT /api/authz/principals/{id}/profiles` binds a profile to a
/// principal. Idempotent. 404 when either the principal or the profile is
/// not configured.
pub async fn handle_bind_principal_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Path(principal_id): Path<String>,
    Json(body): Json<BindProfileBody>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    let profile_id = body.profile_id;
    match write_principal_profiles(
        &state,
        &principal,
        &principal_id,
        Some(&profile_id),
        |mut current| {
            if !current.contains(&profile_id) {
                current.push(profile_id.clone());
            }
            current
        },
    )
    .await
    {
        Ok(profiles) => Json(PrincipalProfilesResponse {
            principal_id,
            profiles,
        })
        .into_response(),
        Err(response) => response,
    }
}

/// `DELETE /api/authz/principals/{id}/profiles?profile_id=<id>` unbinds a
/// profile from a principal. Idempotent: an id that is not bound is a
/// no-op. 404 only when the principal is not configured.
pub async fn handle_unbind_principal_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Path(principal_id): Path<String>,
    Query(q): Query<UnbindProfileQuery>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    match write_principal_profiles(&state, &principal, &principal_id, None, |current| {
        current.into_iter().filter(|p| *p != q.profile_id).collect()
    })
    .await
    {
        Ok(profiles) => Json(PrincipalProfilesResponse {
            principal_id,
            profiles,
        })
        .into_response(),
        Err(response) => response,
    }
}

// ── Principal create / delete ───────────────────────────────────────

/// Principal ids are dotted-path segments (`authz.principals.<id>.*`) and
/// `IdentitySubject::Roster` keys, so they are kept to
/// `[A-Za-z0-9][A-Za-z0-9_.@-]*` and at most 64 bytes. Dots are fine for the
/// natural-key router (it longest-matches live keys).
fn validate_principal_id(id: &str) -> Result<(), ConfigApiError> {
    let mut chars = id.chars();
    let head_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let tail_ok = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '@' | '-'));
    if head_ok && tail_ok && id.len() <= 64 {
        return Ok(());
    }
    Err(ConfigApiError::new(
        ConfigApiCode::InvalidFormat,
        format!(
            "principal id `{id}` is invalid: use letters, digits, `_`, `.`, `@` or `-`, starting with a letter or digit, at most 64 characters"
        ),
    )
    .with_path("authz.principals"))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct CreatePrincipalBody {
    pub id: String,
    #[serde(default)]
    pub profiles: Vec<String>,
}

/// `POST /api/authz/principals` creates a `[[authz.principals]]` row with no
/// bound tokens (tokens are bound by principal-tagged pairing). 409 when the
/// id is already a principal, or already the effective principal id of a
/// `[users.<name>]` entry (config validation requires the two namespaces to
/// be disjoint); 404 when any named profile is not configured. `profiles`
/// may be empty. Answers 201 with the same row shape as the listing.
pub async fn handle_create_principal(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Json(body): Json<CreatePrincipalBody>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    if let Err(e) = validate_principal_id(&body.id) {
        return error_response(e);
    }

    let guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let before = state.config.read().clone();
    let mut working = before.clone();
    let path = format!("authz.principals.{}", body.id);
    if working.authz.by_id(&body.id).is_some() {
        return conflict_response(format!("principal `{}` already exists", body.id), path);
    }
    if let Some(user) = working
        .users
        .iter()
        .find(|(name, u)| u.effective_principal_id(name) == body.id.as_str())
        .map(|(name, _)| name)
    {
        return conflict_response(
            format!(
                "principal id `{}` is already used by [users.{user}]; principal ids must be unique across [users] and [[authz.principals]]",
                body.id
            ),
            path,
        );
    }
    if let Some(profile_id) = body
        .profiles
        .iter()
        .find(|p| !working.permission_profiles.contains_key(p.as_str()))
    {
        return not_found(
            format!("profile `{profile_id}` is not configured"),
            format!("permission_profiles.{profile_id}"),
        );
    }
    if let Err(msg) = working.create_map_key("authz.principals", &body.id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::InternalError, msg).with_path(&path),
        );
    }
    if let Err(e) = set_prop(
        &mut working,
        &format!("{path}.profiles"),
        &json_list(&body.profiles),
    ) {
        return error_response(e);
    }
    if let Err(response) = commit(&state, &principal, &before, working, guard, None).await {
        return response;
    }
    let admin = principal_is_admin(&state.config.read(), &body.id);
    (
        StatusCode::CREATED,
        Json(PrincipalDto {
            id: body.id,
            profiles: body.profiles,
            admin,
            device_ids: Vec::new(),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct DeletePrincipalQuery {
    /// `?force=1` (or `true`) also revokes every bearer bound to the
    /// principal instead of refusing with 409.
    #[serde(default)]
    pub force: Option<String>,
}

impl DeletePrincipalQuery {
    fn force(&self) -> bool {
        matches!(self.force.as_deref().map(str::trim), Some("1" | "true"))
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct DeletePrincipalResponse {
    pub id: String,
    pub deleted: bool,
}

/// `DELETE /api/authz/principals/{id}` removes a `[[authz.principals]]`
/// row. 404 when unknown. 409 while any bearer is still bound to it (inline
/// `token_hashes` or a live `TokenBindingStore` binding) unless `?force=1`,
/// in which case those bearers are revoked from the pairing guard and the
/// binding store as part of the same write: the config row and
/// `gateway.paired_tokens` persist together under the write lock, and the
/// in-memory revocation happens only after that persist succeeds, so a
/// refused or failed write leaves the runtime untouched.
pub async fn handle_delete_principal(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: RequestPrincipal,
    Path(principal_id): Path<String>,
    Query(q): Query<DeletePrincipalQuery>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }

    let guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let before = state.config.read().clone();
    let mut working = before.clone();
    let path = format!("authz.principals.{principal_id}");
    let Some(record) = working.authz.by_id(&principal_id).cloned() else {
        return not_found(
            format!("principal `{principal_id}` is not configured"),
            "authz.principals".to_string(),
        );
    };
    // Live bindings are enumerated (not removed) here; removal waits for
    // the persisted write below.
    let mut bound_hashes = record.token_hashes.clone();
    bound_hashes.extend(
        state.pairing.tokens().into_iter().filter(|hash| {
            state.token_bindings.get(hash).as_deref() == Some(principal_id.as_str())
        }),
    );
    bound_hashes.sort();
    bound_hashes.dedup();
    if !bound_hashes.is_empty() && !q.force() {
        return conflict_response(
            format!(
                "principal `{principal_id}` still has {} bound token(s); revoke them first or pass ?force=1 to revoke them with the principal",
                bound_hashes.len()
            ),
            format!("{path}.token_hashes"),
        );
    }

    if let Err(msg) = working.delete_map_key("authz.principals", &principal_id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::PathNotFound, msg).with_path(&path),
        );
    }
    working.mark_dirty(&path);
    if !bound_hashes.is_empty() {
        // Same write `persist_pairing_tokens` makes (live guard tokens
        // minus the revoked ones), folded into this commit because that
        // helper takes the config write lock itself.
        working.gateway.paired_tokens = state
            .pairing
            .tokens()
            .into_iter()
            .filter(|hash| !bound_hashes.contains(hash))
            .collect();
        working.mark_dirty("gateway.paired_tokens");
    }
    if let Err(response) = commit(
        &state,
        &principal,
        &before,
        working,
        guard,
        Some((path, Verb::Delete)),
    )
    .await
    {
        return response;
    }
    for hash in &bound_hashes {
        state.pairing.revoke_token_hash(hash);
    }
    if let Err(e) = state.token_bindings.remove_principal(&principal_id) {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({
                    "principal_id": principal_id,
                    "error": e.to_string(),
                })),
            "principal deleted but persisting the token binding store failed; its bindings are gone in-process and the deleted principal no longer resolves"
        );
    }
    Json(DeletePrincipalResponse {
        id: principal_id,
        deleted: true,
    })
    .into_response()
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
        let response = handle_list_principals(State(state), bearer_headers(alice)).await;
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

    // ── External (SSO) subjects ─────────────────────────────────────

    fn seen_external(state: &AppState, sub: &str) -> String {
        use zeroclaw_api::principal::{AuthMethod, AuthenticatedIdentity, IdentitySubject};
        use zeroclaw_runtime::security::principal_resolver::OidcMembership;
        let identity = AuthenticatedIdentity::new(
            IdentitySubject::Oidc {
                issuer: "https://sso.example.com/realms/main".into(),
                subject: sub.into(),
            },
            AuthMethod::Oidc,
        )
        .with_provider_alias("corp");
        state
            .external_subjects
            .record_login(
                &identity,
                OidcMembership {
                    groups: vec!["volt-kb".into()],
                    profiles: vec!["kb".into()],
                },
                false,
            )
            .expect("recorded")
            .id
    }

    #[tokio::test]
    async fn list_external_subjects_returns_seen_subjects_for_an_admin() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let id = seen_external(&state, "carol");
        let response =
            handle_list_external_subjects(State(state.clone()), bearer_headers(alice)).await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let subjects = json["subjects"].as_array().expect("subjects array");
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0]["id"], id);
        assert_eq!(subjects[0]["provider"], "corp");
        assert_eq!(subjects[0]["subject"], "carol");
        assert_eq!(subjects[0]["groups"], serde_json::json!(["volt-kb"]));
        assert_eq!(subjects[0]["profiles"], serde_json::json!(["kb"]));
        assert_eq!(subjects[0]["admin"], false);
        assert_eq!(subjects[0]["logins"], 1);
    }

    #[tokio::test]
    async fn forget_external_subject_deletes_and_404s_on_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let id = seen_external(&state, "carol");
        let response = handle_forget_external_subject(
            State(state.clone()),
            bearer_headers(alice),
            Path(id.clone()),
        )
        .await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(json, serde_json::json!({ "id": id, "deleted": true }));
        assert!(state.external_subjects.list().is_empty());

        let response =
            handle_forget_external_subject(State(state), bearer_headers(alice), Path(id)).await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["code"], "path_not_found");
    }

    #[tokio::test]
    async fn external_subjects_are_forbidden_for_a_non_admin() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, bob) = enforced_state(&tmp);
        let id = seen_external(&state, "carol");
        let response =
            handle_list_external_subjects(State(state.clone()), bearer_headers(bob)).await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "forbidden");

        let response =
            handle_forget_external_subject(State(state.clone()), bearer_headers(bob), Path(id))
                .await;
        let (status, _) = response_json(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(state.external_subjects.list().len(), 1, "nothing forgotten");
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

    // ── Profiles CRUD + binding ─────────────────────────────────────

    fn profile_body(id: &str, agents: &[&str], admin: bool) -> Json<ProfileBody> {
        Json(ProfileBody {
            id: id.to_string(),
            allowed_agents: agents.iter().map(|a| a.to_string()).collect(),
            admin,
        })
    }

    async fn create(
        state: &AppState,
        token: &str,
        body: Json<ProfileBody>,
    ) -> (StatusCode, serde_json::Value) {
        response_json(
            handle_create_profile(State(state.clone()), bearer_headers(token), None, body).await,
        )
        .await
    }

    async fn delete(state: &AppState, token: &str, id: &str) -> (StatusCode, serde_json::Value) {
        response_json(
            handle_delete_profile(
                State(state.clone()),
                bearer_headers(token),
                None,
                Query(ProfileIdQuery { id: id.to_string() }),
            )
            .await,
        )
        .await
    }

    async fn bind(
        state: &AppState,
        token: &str,
        principal: &str,
        profile: &str,
    ) -> (StatusCode, serde_json::Value) {
        response_json(
            handle_bind_principal_profile(
                State(state.clone()),
                bearer_headers(token),
                None,
                Path(principal.to_string()),
                Json(BindProfileBody {
                    profile_id: profile.to_string(),
                }),
            )
            .await,
        )
        .await
    }

    async fn unbind(
        state: &AppState,
        token: &str,
        principal: &str,
        profile: &str,
    ) -> (StatusCode, serde_json::Value) {
        response_json(
            handle_unbind_principal_profile(
                State(state.clone()),
                bearer_headers(token),
                None,
                Path(principal.to_string()),
                Query(UnbindProfileQuery {
                    profile_id: profile.to_string(),
                }),
            )
            .await,
        )
        .await
    }

    fn on_disk(state: &AppState) -> String {
        std::fs::read_to_string(&state.config.read().config_path).unwrap_or_default()
    }

    #[tokio::test]
    async fn list_profiles_returns_sorted_panel_shape_for_any_paired_caller() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, bob) = enforced_state(&tmp);
        let (status, json) =
            response_json(handle_list_profiles(State(state), bearer_headers(bob)).await).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            json["profiles"],
            serde_json::json!([
                { "id": "ops", "allowed_agents": [], "admin": true },
                { "id": "viewer", "allowed_agents": ["*"], "admin": false },
            ])
        );
    }

    #[tokio::test]
    async fn create_profile_persists_to_disk_and_live_config() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = create(&state, alice, profile_body("support", &["*"], false)).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            json,
            serde_json::json!({ "id": "support", "allowed_agents": ["*"], "admin": false })
        );
        let live = state.config.read().permission_profiles["support"].clone();
        assert_eq!(live.allowed_agents, vec!["*".to_string()]);
        assert!(!live.admin);
        assert!(
            on_disk(&state).contains("[permission_profiles.support]"),
            "profile must land in config.toml"
        );
    }

    #[tokio::test]
    async fn create_profile_conflicts_on_an_existing_id() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = create(&state, alice, profile_body("viewer", &[], true)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["code"], "conflict");
        assert!(!state.config.read().permission_profiles["viewer"].admin);
    }

    #[tokio::test]
    async fn create_profile_rejects_bad_ids_and_unknown_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        for bad in ["", "-lead", "has space", "dot.ted"] {
            let (status, json) = create(&state, alice, profile_body(bad, &["*"], false)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}: {json}");
            assert_eq!(json["code"], "invalid_format");
        }
        let (status, json) = create(&state, alice, profile_body("ok", &["ghost"], false)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert_eq!(json["code"], "dangling_reference");
        assert_eq!(json["path"], "permission_profiles.ok.allowed_agents");
        assert!(!state.config.read().permission_profiles.contains_key("ok"));
    }

    #[tokio::test]
    async fn update_profile_upserts_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = response_json(
            handle_update_profile(
                State(state.clone()),
                bearer_headers(alice),
                None,
                profile_body("viewer", &[], true),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let live = state.config.read().permission_profiles["viewer"].clone();
        assert!(live.admin);
        assert!(live.allowed_agents.is_empty());
        assert_eq!(state.config.read().permission_profiles.len(), 2);
    }

    #[tokio::test]
    async fn delete_profile_refuses_while_a_principal_is_bound() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = delete(&state, alice, "viewer").await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["code"], "conflict");
        assert!(json["message"].as_str().unwrap().contains("bob"));
        assert!(
            state
                .config
                .read()
                .permission_profiles
                .contains_key("viewer")
        );
    }

    #[tokio::test]
    async fn delete_profile_removes_an_unbound_one_and_404s_on_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, _) = create(&state, alice, profile_body("tmp", &["*"], false)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(on_disk(&state).contains("[permission_profiles.tmp]"));

        let (status, json) = delete(&state, alice, "tmp").await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            json,
            serde_json::json!({ "id": "tmp", "deleted": true, "affected_principals": [] })
        );
        assert!(!state.config.read().permission_profiles.contains_key("tmp"));
        assert!(!on_disk(&state).contains("[permission_profiles.tmp]"));

        let (status, json) = delete(&state, alice, "tmp").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["code"], "path_not_found");
    }

    #[tokio::test]
    async fn bind_profile_is_idempotent_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = bind(&state, alice, "bob", "ops").await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            json,
            serde_json::json!({ "principal_id": "bob", "profiles": ["viewer", "ops"] })
        );
        assert!(principal_is_admin(&state.config.read(), "bob"));
        assert!(on_disk(&state).contains("[[authz.principals]]"));

        let (status, json) = bind(&state, alice, "bob", "ops").await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(json["profiles"], serde_json::json!(["viewer", "ops"]));
    }

    #[tokio::test]
    async fn bind_404s_on_unknown_principal_or_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = bind(&state, alice, "nobody", "ops").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["code"], "path_not_found");
        let (status, json) = bind(&state, alice, "bob", "ghost").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["path"], "permission_profiles.ghost");
        assert_eq!(
            state.config.read().authz.by_id("bob").unwrap().profiles,
            vec!["viewer".to_string()]
        );
    }

    #[tokio::test]
    async fn unbind_profile_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = unbind(&state, alice, "bob", "viewer").await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            json,
            serde_json::json!({ "principal_id": "bob", "profiles": [] })
        );
        assert!(
            state
                .config
                .read()
                .authz
                .by_id("bob")
                .unwrap()
                .profiles
                .is_empty()
        );

        let (status, json) = unbind(&state, alice, "bob", "viewer").await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(json["profiles"], serde_json::json!([]));

        let (status, _) = unbind(&state, alice, "nobody", "viewer").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn mutations_are_forbidden_for_a_non_admin() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, bob) = enforced_state(&tmp);
        let (status, json) = create(&state, bob, profile_body("mine", &["*"], true)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "forbidden");
        let (status, _) = delete(&state, bob, "ops").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = bind(&state, bob, "bob", "ops").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = unbind(&state, bob, "alice", "ops").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let cfg = state.config.read();
        assert!(!cfg.permission_profiles.contains_key("mine"));
        assert!(!principal_is_admin(&cfg, "bob"));
        assert!(principal_is_admin(&cfg, "alice"));
    }

    // ── Principal create / delete ───────────────────────────────────

    async fn create_principal(
        state: &AppState,
        token: &str,
        id: &str,
        profiles: &[&str],
    ) -> (StatusCode, serde_json::Value) {
        response_json(
            handle_create_principal(
                State(state.clone()),
                bearer_headers(token),
                None,
                Json(CreatePrincipalBody {
                    id: id.to_string(),
                    profiles: profiles.iter().map(|p| p.to_string()).collect(),
                }),
            )
            .await,
        )
        .await
    }

    async fn delete_principal(
        state: &AppState,
        token: &str,
        id: &str,
        force: bool,
    ) -> (StatusCode, serde_json::Value) {
        response_json(
            handle_delete_principal(
                State(state.clone()),
                bearer_headers(token),
                None,
                Path(id.to_string()),
                Query(DeletePrincipalQuery {
                    force: force.then(|| "1".to_string()),
                }),
            )
            .await,
        )
        .await
    }

    #[tokio::test]
    async fn create_principal_persists_and_lists_with_panel_shape() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = create_principal(&state, alice, "carol@corp", &["viewer"]).await;
        assert_eq!(status, StatusCode::CREATED, "{json}");
        assert_eq!(
            json,
            serde_json::json!({
                "id": "carol@corp", "profiles": ["viewer"], "admin": false, "device_ids": []
            })
        );
        {
            let cfg = state.config.read();
            let carol = cfg.authz.by_id("carol@corp").expect("carol configured");
            assert_eq!(carol.profiles, vec!["viewer".to_string()]);
            assert!(carol.token_hashes.is_empty());
        }
        assert!(on_disk(&state).contains("carol@corp"));

        let (status, json) = create_principal(&state, alice, "dave", &[]).await;
        assert_eq!(status, StatusCode::CREATED, "{json}");
        assert_eq!(json["profiles"], serde_json::json!([]));

        let (status, json) = response_json(
            handle_list_principals(State(state.clone()), bearer_headers(alice)).await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let ids: Vec<&str> = json["principals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["alice", "bob", "carol@corp", "dave"]);
    }

    #[tokio::test]
    async fn create_principal_with_admin_profile_derives_admin_bit() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, json) = create_principal(&state, alice, "root2", &["ops", "viewer"]).await;
        assert_eq!(status, StatusCode::CREATED, "{json}");
        assert_eq!(json["admin"], true);
        assert!(principal_is_admin(&state.config.read(), "root2"));
    }

    #[tokio::test]
    async fn create_principal_conflicts_on_existing_principal_or_user_id() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        // A valid roster entry: `validate_auth` (run on every admin
        // resolution) requires a uid and a profile, so a bare default
        // would deny alice herself before the handler sees the conflict.
        state.config.write().users.insert(
            "ivan".to_string(),
            zeroclaw_config::schema::UserConfig {
                uid: Some(1000),
                permission_profiles: vec!["viewer".to_string()],
                ..Default::default()
            },
        );
        for taken in ["bob", "ivan"] {
            let (status, json) = create_principal(&state, alice, taken, &[]).await;
            assert_eq!(status, StatusCode::CONFLICT, "{taken}: {json}");
            assert_eq!(json["code"], "conflict");
            assert_eq!(json["path"], format!("authz.principals.{taken}"));
        }
        assert_eq!(
            state.config.read().authz.by_id("bob").unwrap().profiles,
            vec!["viewer".to_string()],
            "a conflicting create must not touch the existing row"
        );
        assert!(state.config.read().authz.by_id("ivan").is_none());
    }

    #[tokio::test]
    async fn create_principal_rejects_bad_ids_and_unknown_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let too_long = "a".repeat(65);
        for bad in ["", "-lead", "has space", "slash/ed", too_long.as_str()] {
            let (status, json) = create_principal(&state, alice, bad, &[]).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}: {json}");
            assert_eq!(json["code"], "invalid_format");
        }
        let (status, json) = create_principal(&state, alice, "erin", &["viewer", "ghost"]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["code"], "path_not_found");
        assert_eq!(json["path"], "permission_profiles.ghost");
        assert!(state.config.read().authz.by_id("erin").is_none());
        assert_eq!(state.config.read().authz.principals.len(), 2);
    }

    #[tokio::test]
    async fn delete_principal_removes_an_unbound_one_and_404s_on_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, _) = enforced_state(&tmp);
        let (status, _) = create_principal(&state, alice, "tmp.user", &["viewer"]).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(on_disk(&state).contains("tmp.user"));

        let (status, json) = delete_principal(&state, alice, "tmp.user", false).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            json,
            serde_json::json!({ "id": "tmp.user", "deleted": true })
        );
        assert!(state.config.read().authz.by_id("tmp.user").is_none());
        assert!(!on_disk(&state).contains("tmp.user"));
        assert!(
            state.config.read().authz.by_id("bob").is_some(),
            "sibling rows survive"
        );

        let (status, json) = delete_principal(&state, alice, "tmp.user", false).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["code"], "path_not_found");
    }

    #[tokio::test]
    async fn delete_principal_refuses_bound_tokens_unless_forced() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, bob) = enforced_state(&tmp);
        let (status, json) = delete_principal(&state, alice, "bob", false).await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["code"], "conflict");
        assert_eq!(json["path"], "authz.principals.bob.token_hashes");
        assert!(state.config.read().authz.by_id("bob").is_some());
        assert!(state.pairing.is_authenticated(bob));

        let (status, json) = delete_principal(&state, alice, "bob", true).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(json["deleted"], true);
        assert!(state.config.read().authz.by_id("bob").is_none());
        assert!(
            !state.pairing.is_authenticated(bob),
            "force must revoke the bearer pinned in token_hashes"
        );
        assert!(state.pairing.is_authenticated(alice));
        assert!(require_admin(&state, &bearer_headers(alice)).await.is_ok());
    }

    #[tokio::test]
    async fn delete_principal_counts_and_revokes_live_bindings() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, alice, bob) = enforced_state(&tmp);
        let carol_token = "carol-tok";
        let state = AppState {
            pairing: std::sync::Arc::new(PairingGuard::new(
                true,
                &[alice.to_string(), bob.to_string(), carol_token.to_string()],
                zeroclaw_config::pairing::PairingCodePolicy::default(),
            )),
            ..state
        };
        let (status, _) = create_principal(&state, alice, "carol", &["viewer"]).await;
        assert_eq!(status, StatusCode::CREATED);
        let carol_hash = PairingGuard::token_hash(carol_token);
        state
            .token_bindings
            .set(carol_hash.clone(), "carol".to_string())
            .unwrap();

        let (status, json) = delete_principal(&state, alice, "carol", false).await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert!(state.pairing.is_authenticated(carol_token));

        let (status, json) = delete_principal(&state, alice, "carol", true).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert!(state.config.read().authz.by_id("carol").is_none());
        assert!(state.token_bindings.get(&carol_hash).is_none());
        assert!(!state.pairing.is_authenticated(carol_token));
        assert!(state.pairing.is_authenticated(bob));
    }

    #[tokio::test]
    async fn principal_mutations_are_forbidden_for_a_non_admin() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, _, bob) = enforced_state(&tmp);
        let (status, json) = create_principal(&state, bob, "mine", &["ops"]).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "forbidden");
        let (status, _) = delete_principal(&state, bob, "alice", true).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let cfg = state.config.read();
        assert!(cfg.authz.by_id("mine").is_none());
        assert!(cfg.authz.by_id("alice").is_some());
    }
}
