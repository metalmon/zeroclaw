//! F4's admin control plane: CRUD for `[[authz.profiles]]` and
//! principal-to-profile binding, plus the shared admin gate every
//! config-mutating REST route (this module's own mutations AND
//! `api_config.rs`'s config-write surface) is wired through.
//!
//! Security-critical: without this gate, ANY paired device could grant
//! itself every permission by editing config directly (self-escalation).
//! `require_admin` is the single chokepoint that closes that hole —
//! `crate::api_config` calls it too, right after its existing
//! `require_auth` (paired-guard) check.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zeroclaw_config::api_error::{ConfigApiCode, ConfigApiError};

use super::AppState;
use super::api::require_auth;
use super::api_config::{map_prop_error, persist_and_swap};

// ── Admin gate ──────────────────────────────────────────────────────

/// Resolve the caller's principal for an admin-gated REST route — the same
/// way `acp::resolve_principal` resolves an `/acp` connection's bearer — and
/// require `config.authz.is_admin(principal.id)`.
///
/// Lockout fallback (CRITICAL): while authz is UNCONFIGURED (no
/// `[[authz.principals]]` / `[[authz.profiles]]` exist anywhere in config,
/// i.e. `!authz.is_enforced()`), this delegates entirely to
/// [`require_auth`] (the existing paired-guard) instead of demanding an
/// admin principal. A fresh install has no principals yet, so requiring
/// `is_admin` unconditionally would lock the operator out of their own
/// control plane. The instant any principal or profile is configured, this
/// fallback stops applying: an unresolved credential, a credential the auth
/// provider denies, or a credential that resolves to a principal not bound
/// to any `admin` profile are all a flat 403 from here on — UNLESS the
/// bootstrap-operator rescue below applies (Task 4: see its doc comment).
///
/// Only bearer-token resolution is wired for REST today (mirrors
/// `require_auth`'s bearer-only paired-guard) — the mTLS `device_id` leg
/// `acp::resolve_principal` also accepts is ACP-connection-specific
/// (`Extension<ClientDeviceId>`) and not threaded through this REST
/// surface; see the task report for the scoping rationale.
/// The gate's error type deliberately mirrors [`require_auth`]'s own
/// `Result<(), (StatusCode, Json<serde_json::Value>)>` shape instead of the
/// full `Response` — `clippy::result_large_err` (denied in CI) flags any
/// `Result<_, Response>` because `Response` carries a `HeaderMap` and is far
/// past the lint's size threshold. This tuple is small (proven: it's the
/// exact type `require_auth` already returns everywhere in the crate) and
/// the unconfigured-authz fallback below returns it straight through with
/// no conversion at all.
pub(crate) async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if !state.config.read().authz.is_enforced() {
        return require_auth(state, headers);
    }

    let token = super::api::extract_bearer_token(headers);
    let principal = crate::acp::resolve_principal(&state.provider_registry, token, None).await;

    let is_admin = principal
        .as_ref()
        .is_some_and(|p| state.config.read().authz.is_admin(p.id.as_str()));

    if is_admin {
        return Ok(());
    }

    // F4 bootstrap-operator rescue (Task 4). `resolve_principal` above
    // answers off `state.provider_registry`'s `PairingAuthProvider`, which
    // holds an `AuthzConfig` SNAPSHOT frozen at daemon start (or the last
    // `/admin/reload`) — it is never rebuilt by a bare config write. So the
    // instant enforcement turns on LIVE (the very write that creates the
    // first `[[authz.principals]]`/`[[authz.profiles]]`, e.g. via
    // `handle_create_profile` below), every bearer still resolves through
    // that STALE, still-*unenforced* snapshot: `PairingAuthProvider::verify`
    // short-circuits on its own `!self.authz.is_enforced()` check and
    // returns `Trusted(Principal::shared_operator())` for ANY non-empty
    // bearer, never a real configured principal, until the next
    // reload/restart rebuilds the registry. The ordinary `is_admin(p.id)`
    // check above can therefore never observe
    // `AuthzConfig::seed_operator_admin_if_locked_out`'s seed in that
    // window, no matter what id it's under.
    //
    // This rescue closes that window by bypassing the stale resolver
    // entirely rather than trusting it: it hashes the CALLER'S OWN bearer
    // and looks THAT SPECIFIC hash up directly against the LIVE config
    // (`AuthzConfig::lookup`, not `resolve_principal`), then checks whether
    // the principal it resolves to (if any) is admin-bound.
    //
    // This is deliberately NOT "is this bearer merely paired" (an earlier
    // version used `require_auth` here, which checks paired-set membership
    // only — ANY currently-paired device would pass, including one the
    // operator explicitly configured as a non-admin role, letting it
    // self-promote). A live hash-for-hash lookup is strictly narrower: it
    // only ever succeeds for a credential some LIVE principal's
    // `token_hashes` explicitly names, and that principal's own `is_admin`
    // status governs the outcome like anywhere else — a non-admin
    // principal's own token still gets denied here, exactly as it would
    // once `resolve_principal` catches up after the next reload.
    let caller_hash = token.map(zeroclaw_config::pairing::PairingGuard::token_hash);
    let rescued = caller_hash.as_deref().is_some_and(|hash| {
        let cfg = state.config.read();
        cfg.authz
            .lookup(hash, None)
            .is_some_and(|rec| cfg.authz.is_admin(&rec.id))
    });
    if rescued {
        return Ok(());
    }

    Err(forbidden_error())
}

/// If this profile write is the one turning authz enforcement on (or
/// enforcement is already on but the caller's own credential has no admin
/// path yet), seed the operator-bootstrap admin path scoped to THIS
/// CALLER'S OWN bearer only.
///
/// Deliberately narrow: it hashes only the token presented on THIS request
/// and passes that single hash to
/// [`zeroclaw_config::authz::AuthzConfig::seed_operator_admin_if_locked_out`]
/// — never `gateway.paired_tokens` or any other set of credentials. The
/// caller reaching this point has already passed [`require_admin`] (either
/// via its pre-enforcement paired-guard fallback, or as an existing admin),
/// so treating THIS specific credential as "the configuring operator" for
/// bootstrap purposes is exactly the carried ruling's scope — no other
/// currently-paired device is touched. If the caller presented no bearer,
/// or their hash already resolves to any existing principal (admin or
/// not — see the callee's own exclusion invariant), this is a no-op.
///
/// Persistence: rides the SAME `save_dirty` write the caller is already
/// about to do for the profile it's creating/updating — no separate
/// mechanism. `seed_operator_admin_if_locked_out` mutates `working.authz`
/// directly (`self.principals.push`/`self.profiles.push`, not through
/// `create_map_key`), so on its own the mutation would be swapped into live
/// `state.config` by `persist_and_swap` but never reach `config.toml`: only
/// paths in `Config::dirty_paths` get written by `save_dirty`. When the
/// seed reports it actually changed something, this explicitly
/// `mark_dirty`s the exact leaf paths it touched. That alone is enough —
/// `Config::save_dirty`'s natural-key writer (`ensure_array_of_tables_entry`)
/// creates a brand-new `[[authz.principals]]` / `[[authz.profiles]]` row
/// from the live in-memory element and seeds its natural-key column
/// (`id`) automatically the first time ANY of its fields is dirty; no
/// `create_map_key` call is needed since the element already exists in
/// `working.authz` (pushed above). `allowed_agents` / `device_ids` are left
/// unmarked: both are `#[serde(default)]` and stay empty for this seed, so
/// omitting them from the written TOML round-trips identically to writing
/// an empty array.
fn seed_operator_admin_for_caller(
    working: &mut zeroclaw_config::schema::Config,
    headers: &HeaderMap,
) {
    let Some(token) = super::api::extract_bearer_token(headers) else {
        return;
    };
    let hash = zeroclaw_config::pairing::PairingGuard::token_hash(token);
    let seeded = working.authz.seed_operator_admin_if_locked_out(&[hash]);
    if !seeded {
        return;
    }
    working.mark_dirty(&format!(
        "authz.profiles.{}.admin",
        zeroclaw_config::authz::OPERATOR_ADMIN_PROFILE_ID
    ));
    working.mark_dirty(&format!(
        "authz.principals.{}.token_hashes",
        zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID
    ));
    working.mark_dirty(&format!(
        "authz.principals.{}.profiles",
        zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID
    ));
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

fn error_response(err: ConfigApiError) -> Response {
    let status =
        StatusCode::from_u16(err.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(err)).into_response()
}

// ── Profiles CRUD ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfileDto {
    pub id: String,
    pub allowed_agents: Vec<String>,
    pub admin: bool,
}

impl From<&zeroclaw_config::authz::PermissionProfile> for ProfileDto {
    fn from(p: &zeroclaw_config::authz::PermissionProfile) -> Self {
        Self {
            id: p.id.clone(),
            allowed_agents: p.allowed_agents.clone(),
            admin: p.admin,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfilesListResponse {
    pub profiles: Vec<ProfileDto>,
}

/// `GET /api/authz/profiles` — list every configured `[[authz.profiles]]`
/// entry. Read-only, gated by the existing paired-guard (`require_auth`)
/// rather than the admin gate: enumerating profile shape is not itself a
/// mutation, matching every other `GET /api/config/*` listing endpoint.
pub async fn handle_list_profiles(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    let cfg = state.config.read().clone();
    let profiles = cfg.authz.profiles.iter().map(ProfileDto::from).collect();
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

fn validate_profile_id(id: &str) -> Result<(), ConfigApiError> {
    if id.trim().is_empty() {
        return Err(ConfigApiError::new(
            ConfigApiCode::RequiredFieldEmpty,
            "profile `id` is required",
        )
        .with_path("authz.profiles"));
    }
    Ok(())
}

/// Work around a `zeroclaw-macros` `create_map_key` limitation, discovered
/// while verifying Task 4's operator-bootstrap persistence: its insertion
/// logic only auto-populates a freshly-created element's `name` or `hint`
/// field from the supplied key — both hardcoded, never the struct's actual
/// `#[natural_key = "..."]` field name. `PermissionProfile`'s natural key is
/// `id` (neither `name` nor `hint`), so a brand-new `[[authz.profiles]]`
/// element gets pushed with `id` left at its serde default (`""`) instead
/// of the supplied `id`. `create_map_key` itself still reports `Ok(true)`;
/// the corruption is silent until the very next call, when EVERY
/// `set_prop`/`set_prop_persistent` into that element's dotted path fails
/// with "Unknown property" — `route_vec_path` can't find an element whose
/// `id` matches the alias, because none does.
///
/// Only call this immediately after a `create_map_key` call that itself
/// reported `Ok(true)` (a new element WAS pushed): that guarantees the
/// element `.last_mut()` refers to is the one just created, with its `id`
/// still blank — an upsert that matched an EXISTING element (`Ok(false)`)
/// must not go through this path, since `.last_mut()` would then likely
/// refer to a different, unrelated element.
fn fixup_created_profile_natural_key(working: &mut zeroclaw_config::schema::Config, id: &str) {
    if let Some(profile) = working.authz.profiles.last_mut() {
        if profile.id.is_empty() {
            profile.id = id.to_string();
        }
    }
}

/// Write `body`'s `allowed_agents` and `admin` onto the (already-created)
/// `authz.profiles.<id>` record via the standard `set_prop_persistent`
/// dotted-path engine — the same field-write machinery every other
/// map-key section in `api_config.rs` uses, so `mark_dirty` + the TOML
/// writer see these edits exactly like a `config.toml` hand-edit would.
fn apply_profile_fields(
    working: &mut zeroclaw_config::schema::Config,
    body: &ProfileBody,
) -> Result<(), ConfigApiError> {
    let agents_json =
        serde_json::to_string(&body.allowed_agents).unwrap_or_else(|_| "[]".to_string());
    let agents_path = format!("authz.profiles.{}.allowed_agents", body.id);
    working
        .set_prop_persistent(&agents_path, &agents_json)
        .map_err(|e| map_prop_error(e, &agents_path))?;

    let admin_path = format!("authz.profiles.{}.admin", body.id);
    working
        .set_prop_persistent(&admin_path, if body.admin { "true" } else { "false" })
        .map_err(|e| map_prop_error(e, &admin_path))?;
    Ok(())
}

/// `POST /api/authz/profiles` — create a new permission profile. 409
/// `conflict` if `id` is already taken (use `PUT` to update it instead).
pub async fn handle_create_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ProfileBody>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    if let Err(e) = validate_profile_id(&body.id) {
        return error_response(e);
    }

    let _cfg_guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let mut working = state.config.read().clone();
    if working.authz.profiles.iter().any(|p| p.id == body.id) {
        return error_response(
            ConfigApiError::new(
                ConfigApiCode::Conflict,
                format!(
                    "profile `{}` already exists — use PUT to update it",
                    body.id
                ),
            )
            .with_path(format!("authz.profiles.{}", body.id)),
        );
    }
    match working.create_map_key("authz.profiles", &body.id) {
        Ok(created) => {
            if created {
                fixup_created_profile_natural_key(&mut working, &body.id);
            }
        }
        Err(msg) => {
            return error_response(
                ConfigApiError::new(ConfigApiCode::InternalError, msg).with_path("authz.profiles"),
            );
        }
    }
    if let Err(e) = apply_profile_fields(&mut working, &body) {
        return error_response(e);
    }
    seed_operator_admin_for_caller(&mut working, &headers);
    if let Err(e) = persist_and_swap(&state, working, &_cfg_guard).await {
        return error_response(e);
    }
    Json(ProfileDto {
        id: body.id,
        allowed_agents: body.allowed_agents,
        admin: body.admin,
    })
    .into_response()
}

/// `PUT /api/authz/profiles` — create-or-update a permission profile
/// (idempotent upsert, keyed by `id` in the body).
pub async fn handle_update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ProfileBody>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    if let Err(e) = validate_profile_id(&body.id) {
        return error_response(e);
    }

    let _cfg_guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let mut working = state.config.read().clone();
    match working.create_map_key("authz.profiles", &body.id) {
        Ok(created) => {
            if created {
                fixup_created_profile_natural_key(&mut working, &body.id);
            }
        }
        Err(msg) => {
            return error_response(
                ConfigApiError::new(ConfigApiCode::InternalError, msg).with_path("authz.profiles"),
            );
        }
    }
    if let Err(e) = apply_profile_fields(&mut working, &body) {
        return error_response(e);
    }
    seed_operator_admin_for_caller(&mut working, &headers);
    if let Err(e) = persist_and_swap(&state, working, &_cfg_guard).await {
        return error_response(e);
    }
    Json(ProfileDto {
        id: body.id,
        allowed_agents: body.allowed_agents,
        admin: body.admin,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ProfileIdQuery {
    pub id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct DeleteProfileResponse {
    pub id: String,
    pub deleted: bool,
    /// Principal ids still bound to the deleted profile's `id` via
    /// `PrincipalRecord::profiles`. Deleting a profile does NOT scrub these
    /// bindings — this is fail-closed BY DESIGN: a dangling profile-id
    /// reference resolves to nothing in `AuthzConfig::effective_agents` /
    /// `AuthzConfig::is_admin` (both `filter_map` over `profile_by_id`,
    /// silently dropping an id that no longer resolves), never promoted to
    /// "all agents" or "admin". Listed here so the operator can rebind the
    /// affected principals to a real profile.
    pub affected_principals: Vec<String>,
}

/// `DELETE /api/authz/profiles?id=<profile_id>` — remove a permission
/// profile. Fail-closed: any principal still bound to it is left with a
/// dangling reference that grants nothing, never silently promoted or
/// auto-unbound. Reports the affected principal ids in the response.
pub async fn handle_delete_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ProfileIdQuery>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }

    let _cfg_guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let mut working = state.config.read().clone();
    if !working.authz.profiles.iter().any(|p| p.id == q.id) {
        return error_response(
            ConfigApiError::new(
                ConfigApiCode::PathNotFound,
                format!("profile `{}` is not configured", q.id),
            )
            .with_path(format!("authz.profiles.{}", q.id)),
        );
    }

    // Snapshot BEFORE the delete: these principals keep referencing `q.id`
    // in their own `profiles` list — that list is never scrubbed here.
    let affected_principals: Vec<String> = working
        .authz
        .principals
        .iter()
        .filter(|p| p.profiles.iter().any(|pid| pid == &q.id))
        .map(|p| p.id.clone())
        .collect();

    if let Err(msg) = working.delete_map_key("authz.profiles", &q.id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::PathNotFound, msg).with_path("authz.profiles"),
        );
    }
    working.mark_dirty(&format!("authz.profiles.{}", q.id));
    if let Err(e) = persist_and_swap(&state, working, &_cfg_guard).await {
        return error_response(e);
    }
    Json(DeleteProfileResponse {
        id: q.id,
        deleted: true,
        affected_principals,
    })
    .into_response()
}

// ── Principal <-> profile binding ────────────────────────────────────

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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct PrincipalProfilesResponse {
    pub principal_id: String,
    pub profiles: Vec<String>,
}

/// Look up `principal_id` in `working.authz.principals` and return
/// `(exists, its current `profiles` list clone)`. Cloning up front (rather
/// than holding the `&PrincipalRecord` borrow across the later `&mut
/// working` write) keeps the borrow checker happy without unsafe tricks.
fn current_principal_profiles(
    working: &zeroclaw_config::schema::Config,
    principal_id: &str,
) -> (bool, Vec<String>) {
    match working
        .authz
        .principals
        .iter()
        .find(|p| p.id == principal_id)
    {
        Some(p) => (true, p.profiles.clone()),
        None => (false, Vec::new()),
    }
}

fn principal_not_found(principal_id: &str) -> Response {
    error_response(
        ConfigApiError::new(
            ConfigApiCode::PathNotFound,
            format!("principal `{principal_id}` is not configured"),
        )
        .with_path("authz.principals"),
    )
}

/// `PUT /api/authz/principals/{id}/profiles` — bind a profile to a
/// principal. Idempotent: re-binding an already-bound `profile_id` is a
/// no-op that still returns the current state. 404 if the principal itself
/// is not configured (principals are seeded by pairing / Task 4's
/// bootstrap, not created by this endpoint). The named profile need NOT
/// already exist — binding a not-yet-created id is allowed and simply
/// contributes nothing until that profile is created, mirroring the same
/// fail-closed unresolved-id handling `effective_agents` already applies.
pub async fn handle_bind_principal_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(principal_id): Path<String>,
    Json(body): Json<BindProfileBody>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }

    let _cfg_guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let mut working = state.config.read().clone();
    let (exists, current) = current_principal_profiles(&working, &principal_id);
    if !exists {
        return principal_not_found(&principal_id);
    }
    if current.iter().any(|p| p == &body.profile_id) {
        return Json(PrincipalProfilesResponse {
            principal_id,
            profiles: current,
        })
        .into_response();
    }

    let mut new_profiles = current;
    new_profiles.push(body.profile_id);
    let profiles_json = serde_json::to_string(&new_profiles).unwrap_or_else(|_| "[]".to_string());
    let path = format!("authz.principals.{principal_id}.profiles");
    if let Err(e) = working.set_prop_persistent(&path, &profiles_json) {
        return error_response(map_prop_error(e, &path));
    }
    if let Err(e) = persist_and_swap(&state, working, &_cfg_guard).await {
        return error_response(e);
    }
    Json(PrincipalProfilesResponse {
        principal_id,
        profiles: new_profiles,
    })
    .into_response()
}

/// `DELETE /api/authz/principals/{id}/profiles?profile_id=<id>` — unbind a
/// profile from a principal. Idempotent: unbinding an id that isn't
/// currently bound is a no-op, not an error. 404 only if the principal
/// itself is not configured.
pub async fn handle_unbind_principal_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(principal_id): Path<String>,
    Query(q): Query<UnbindProfileQuery>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }

    let _cfg_guard = Arc::clone(&state.config_write_lock).lock_owned().await;
    let mut working = state.config.read().clone();
    let (exists, current) = current_principal_profiles(&working, &principal_id);
    if !exists {
        return principal_not_found(&principal_id);
    }

    let new_profiles: Vec<String> = current.into_iter().filter(|p| p != &q.profile_id).collect();
    let profiles_json = serde_json::to_string(&new_profiles).unwrap_or_else(|_| "[]".to_string());
    let path = format!("authz.principals.{principal_id}.profiles");
    if let Err(e) = working.set_prop_persistent(&path, &profiles_json) {
        return error_response(map_prop_error(e, &path));
    }
    if let Err(e) = persist_and_swap(&state, working, &_cfg_guard).await {
        return error_response(e);
    }
    Json(PrincipalProfilesResponse {
        principal_id,
        profiles: new_profiles,
    })
    .into_response()
}

// ── Agent alias picker ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct AgentsListResponse {
    pub agents: Vec<String>,
}

/// `GET /api/agents` — the full configured agent-alias list, for the
/// authz-admin profile picker's `allowed_agents` multi-select. Unfiltered
/// (every configured alias, admin-surface listing) — distinct from the
/// principal-scoped `GET /api/config/agent-options` list the dashboard's
/// own per-principal agent picker uses.
pub async fn handle_list_agents(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    use zeroclaw_config::traits::AliasSource;
    let cfg = state.config.read().clone();
    let agents = cfg.resolve_alias_source(AliasSource::Agents);
    Json(AgentsListResponse { agents }).into_response()
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GatewayRateLimiter, IdempotencyStore, nodes};
    use async_trait::async_trait;
    use axum::http::{HeaderValue, header};
    use http_body_util::BodyExt;
    use parking_lot::RwLock;
    use std::time::Duration;
    use zeroclaw_config::authz::{AuthzConfig, PermissionProfile, PrincipalRecord};
    use zeroclaw_config::pairing::PairingGuard as ConfigPairingGuard;
    use zeroclaw_providers::ModelProvider;
    use zeroclaw_runtime::security::pairing::PairingGuard;

    #[derive(Default)]
    struct MockModelProvider;

    #[async_trait]
    impl ModelProvider for MockModelProvider {
        async fn chat_with_system(
            &self,
            _system_prompt: Option<&str>,
            _message: &str,
            _model: &str,
            _temperature: Option<f64>,
        ) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    impl ::zeroclaw_api::attribution::Attributable for MockModelProvider {
        fn role(&self) -> ::zeroclaw_api::attribution::Role {
            ::zeroclaw_api::attribution::Role::Provider(
                ::zeroclaw_api::attribution::ProviderKind::Model(
                    ::zeroclaw_api::attribution::ModelProviderKind::Custom,
                ),
            )
        }

        fn alias(&self) -> &str {
            "MockModelProvider"
        }
    }

    fn temp_config(tmp: &tempfile::TempDir, authz: AuthzConfig) -> zeroclaw_config::schema::Config {
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        zeroclaw_config::schema::Config {
            config_path: tmp.path().join("config.toml"),
            data_dir,
            authz,
            ..Default::default()
        }
    }

    /// Build an `AppState` whose `provider_registry` resolves bearer tokens
    /// against `config.authz` (same content, so identity resolution and the
    /// live `is_admin` check agree) and whose paired-guard requires a token
    /// matching `operator_token` — the lockout-fallback path's "the operator
    /// is already paired" precondition.
    fn test_state(config: zeroclaw_config::schema::Config, operator_token: &str) -> AppState {
        let memory: Arc<dyn zeroclaw_memory::Memory> =
            Arc::new(zeroclaw_memory::NoneMemory::new("api-authz-test"));
        let authz = config.authz.clone();
        AppState {
            config: Arc::new(RwLock::new(config)),
            config_write_lock: Arc::new(tokio::sync::Mutex::new(())),
            model_provider: Arc::new(MockModelProvider),
            model: "test-model".into(),
            temperature: None,
            mem: memory.clone(),
            memory_strategy: Arc::new(
                zeroclaw_runtime::agent::memory_strategy::DefaultMemoryStrategy::with_config(
                    memory,
                    zeroclaw_config::schema::MemoryConfig::default(),
                    std::path::PathBuf::new(),
                ),
            ),
            auto_save: false,
            pairing: Arc::new(PairingGuard::new(true, &[operator_token.to_string()])),
            trust_forwarded_headers: false,
            rate_limiter: Arc::new(GatewayRateLimiter::new(100, 100, 100)),
            auth_limiter: Arc::new(crate::auth_rate_limit::AuthRateLimiter::new()),
            idempotency_store: Arc::new(IdempotencyStore::new(Duration::from_secs(300), 1000)),
            #[cfg(feature = "channel-whatsapp-cloud")]
            whatsapp: std::collections::HashMap::new(),
            #[cfg(feature = "channel-whatsapp-cloud")]
            whatsapp_app_secret: std::collections::HashMap::new(),
            #[cfg(feature = "channel-linq")]
            linq: std::collections::HashMap::new(),
            #[cfg(feature = "channel-linq")]
            linq_signing_secrets: std::collections::HashMap::new(),
            #[cfg(feature = "channel-nextcloud")]
            nextcloud_talk: std::collections::HashMap::new(),
            #[cfg(feature = "channel-nextcloud")]
            nextcloud_talk_webhook_secret: std::collections::HashMap::new(),
            #[cfg(feature = "channel-email")]
            gmail_push: None,
            observer: Arc::new(zeroclaw_runtime::observability::NoopObserver),
            tools_registry: Arc::new(Vec::new()),
            tools_registry_by_agent: Arc::new(std::collections::HashMap::new()),
            cost_tracker: None,
            event_tx: tokio::sync::broadcast::channel(16).0,
            event_buffer: Arc::new(crate::sse::EventBuffer::new(16)),
            shutdown_tx: tokio::sync::watch::channel(false).0,
            reload_tx: None,
            node_registry: Arc::new(nodes::NodeRegistry::new(16)),
            mdns_peer_registry: nodes::mdns::MdnsPeerRegistry::default(),
            path_prefix: String::new(),
            web_dist_dir: None,
            session_backend: None,
            session_queue: Arc::new(crate::session_queue::SessionActorQueue::new(8, 30, 600)),
            device_registry: None,
            pending_pairings: None,
            canvas_store: zeroclaw_runtime::tools::CanvasStore::new(),
            #[cfg(feature = "webauthn")]
            webauthn: None,
            cancel_tokens: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            pending_reload: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tui_registry: None,
            sop_engine: None,
            sop_audit: None,
            provider_registry: crate::acp::build_provider_registry(
                authz,
                std::sync::Arc::new(zeroclaw_config::authz::TokenBindingStore::new_ephemeral()),
            ),
            token_bindings: std::sync::Arc::new(
                zeroclaw_config::authz::TokenBindingStore::new_ephemeral(),
            ),
        }
    }

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
        let body = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        let json = serde_json::from_slice(&body).expect("valid json response");
        (status, json)
    }

    fn admin_authz(admin_token: &str, admin_id: &str) -> AuthzConfig {
        AuthzConfig {
            principals: vec![PrincipalRecord {
                id: admin_id.to_string(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec![ConfigPairingGuard::token_hash(admin_token)],
                profiles: vec!["ops".to_string()],
            }],
            profiles: vec![PermissionProfile {
                id: "ops".to_string(),
                allowed_agents: vec![],
                admin: true,
            }],
        }
    }

    fn non_admin_authz(token: &str, principal_id: &str) -> AuthzConfig {
        AuthzConfig {
            principals: vec![PrincipalRecord {
                id: principal_id.to_string(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec![ConfigPairingGuard::token_hash(token)],
                profiles: vec!["crm".to_string()],
            }],
            profiles: vec![PermissionProfile {
                id: "crm".to_string(),
                allowed_agents: vec!["crm-bot".to_string()],
                admin: false,
            }],
        }
    }

    // ── require_admin ────────────────────────────────────────────────

    #[tokio::test]
    async fn unconfigured_authz_falls_back_to_paired_guard_for_the_operator() {
        // No `[[authz.principals]]` / `[[authz.profiles]]` anywhere: the
        // lockout-fallback ruling must let the already-paired operator
        // through without an admin-bound principal.
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(temp_config(&tmp, AuthzConfig::default()), "operator-tok");
        assert!(!state.config.read().authz.is_enforced());

        let result = require_admin(&state, &bearer_headers("operator-tok")).await;
        assert!(
            result.is_ok(),
            "unconfigured authz must fall back to the paired-guard, not demand admin"
        );
    }

    #[tokio::test]
    async fn unconfigured_authz_still_rejects_an_unpaired_caller() {
        // The fallback is to the paired-guard, not to "allow everyone" —
        // an unpaired bearer is still rejected even with authz unconfigured.
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(temp_config(&tmp, AuthzConfig::default()), "operator-tok");
        let result = require_admin(&state, &bearer_headers("wrong-token")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn configured_authz_denies_a_non_admin_principal() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = non_admin_authz("alice-tok", "alice");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");
        assert!(state.config.read().authz.is_enforced());

        let response = handle_create_profile(
            State(state),
            bearer_headers("alice-tok"),
            Json(ProfileBody {
                id: "new-profile".into(),
                allowed_agents: vec![],
                admin: false,
            }),
        )
        .await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "forbidden");
    }

    #[tokio::test]
    async fn configured_authz_allows_an_admin_principal() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        let response = handle_create_profile(
            State(state.clone()),
            bearer_headers("bob-tok"),
            Json(ProfileBody {
                id: "new-profile".into(),
                allowed_agents: vec!["crm-bot".into()],
                admin: false,
            }),
        )
        .await;
        let (status, _json) = response_json(response).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            state
                .config
                .read()
                .authz
                .profiles
                .iter()
                .any(|p| p.id == "new-profile")
        );
    }

    #[tokio::test]
    async fn configured_authz_denies_an_unresolved_credential() {
        // Enforced authz, but the presented bearer maps to nothing at all
        // (not even a non-admin principal) — must still be a flat 403, not
        // a panic or an accidental fallback.
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        let response = handle_create_profile(
            State(state),
            bearer_headers("nobody-tok"),
            Json(ProfileBody {
                id: "new-profile".into(),
                allowed_agents: vec![],
                admin: false,
            }),
        )
        .await;
        let (status, _json) = response_json(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // ── Task 4: operator bootstrap (don't lock out the operator) ──────

    /// Build a config whose only authz-relevant state is a single bootstrap
    /// pairing token — the pre-authz "install, pair, done" state, exactly
    /// what `gateway.paired_tokens` holds before anyone has ever touched
    /// `[[authz.principals]]` / `[[authz.profiles]]`.
    fn bootstrap_config(
        tmp: &tempfile::TempDir,
        operator_token: &str,
    ) -> zeroclaw_config::schema::Config {
        let mut cfg = temp_config(tmp, AuthzConfig::default());
        cfg.gateway.paired_tokens = vec![ConfigPairingGuard::token_hash(operator_token)];
        cfg
    }

    #[tokio::test]
    async fn creating_first_profile_seeds_operator_admin_so_the_operator_keeps_admin_access() {
        // Bootstrap state: authz UNCONFIGURED, but the operator already has
        // a paired bearer token -- the pre-authz shared-operator credential
        // this ruling exists to protect. This is the "install, pair, THEN
        // enable authz" sequence: nothing in `[[authz.principals]]` /
        // `[[authz.profiles]]` yet.
        let tmp = tempfile::tempdir().unwrap();
        let operator_token = "operator-tok";
        let state = test_state(bootstrap_config(&tmp, operator_token), operator_token);
        assert!(!state.config.read().authz.is_enforced());
        assert!(
            require_admin(&state, &bearer_headers(operator_token))
                .await
                .is_ok(),
            "before enforcement, the operator passes via the paired-guard fallback"
        );

        // The operator (still only recognized via that fallback) creates the
        // very first profile -- the exact write that flips `is_enforced()`
        // to true and, without this seed, would strand every subsequent
        // admin call in the SAME process.
        let response = handle_create_profile(
            State(state.clone()),
            bearer_headers(operator_token),
            Json(ProfileBody {
                id: "ops".into(),
                allowed_agents: vec![],
                admin: true,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(state.config.read().authz.is_enforced());

        // The operator's very next admin call -- same process, no restart,
        // same bearer token -- must still succeed.
        let result = require_admin(&state, &bearer_headers(operator_token)).await;
        assert!(
            result.is_ok(),
            "operator must not be locked out the instant the first profile is created"
        );

        // And the seed landed in live config, under the well-known id.
        assert!(
            state
                .config
                .read()
                .authz
                .is_admin(zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID),
            "the well-known operator principal must be admin-bound"
        );
    }

    #[tokio::test]
    async fn operator_bootstrap_seed_is_persisted_to_disk_not_just_live_state() {
        // The residual the coordinator's follow-up review flagged: the seed
        // must ride the SAME `save_dirty` write the profile creation itself
        // does, not merely land in live `state.config`. Otherwise a restart
        // before any other config write drops the seed entirely --
        // config-load only WARNS on a locked-out operator, it does not
        // re-seed (see `AuthzConfig::bootstrap_operator_is_locked_out`) --
        // stranding the operator again on the very next boot.
        let tmp = tempfile::tempdir().unwrap();
        let operator_token = "operator-tok";
        let cfg = bootstrap_config(&tmp, operator_token);
        let config_path = cfg.config_path.clone();
        let state = test_state(cfg, operator_token);

        let response = handle_create_profile(
            State(state.clone()),
            bearer_headers(operator_token),
            Json(ProfileBody {
                id: "ops".into(),
                allowed_agents: vec![],
                admin: true,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let on_disk =
            std::fs::read_to_string(&config_path).expect("config.toml must exist after the write");
        assert!(
            on_disk.contains("ops"),
            "the operator-created profile itself must be on disk:\n{on_disk}"
        );
        assert!(
            on_disk.contains(zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID),
            "the seeded operator principal must be on disk, not live-only:\n{on_disk}"
        );
        assert!(
            on_disk.contains(zeroclaw_config::authz::OPERATOR_ADMIN_PROFILE_ID),
            "the seeded operator-admin profile must be on disk, not live-only:\n{on_disk}"
        );
        let operator_hash = ConfigPairingGuard::token_hash(operator_token);
        assert!(
            on_disk.contains(&operator_hash),
            "the operator's own token hash must be persisted under the seeded principal:\n{on_disk}"
        );

        // Round-trip proof, simulating a restart: reload the SAME file from
        // scratch and confirm the seeded principal resolves normally via
        // its `token_hashes` pin -- no live rescue needed post-restart.
        let reloaded = zeroclaw_config::migration::migrate_to_current(&on_disk)
            .expect("persisted config.toml must reparse cleanly");
        assert!(
            reloaded
                .authz
                .is_admin(zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID),
            "reloading the persisted file from scratch must still resolve the operator as admin"
        );
        assert_eq!(
            reloaded
                .authz
                .lookup(&operator_hash, None)
                .map(|p| p.id.clone()),
            Some(zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID.to_string()),
            "the operator's hash must resolve via normal AuthzConfig::lookup after \
             reload, not just the live rescue"
        );
    }

    #[tokio::test]
    async fn bootstrap_rescue_denies_a_caller_without_a_real_paired_token() {
        // Same transition as above, but the second caller presents an
        // arbitrary bearer that was never bound to anything. `resolve_principal`
        // (still answering off the frozen, pre-enforcement snapshot) would
        // happily call this `Trusted(shared_operator)` too -- the rescue
        // must NOT let that alone grant admin: its own live hash-for-hash
        // lookup against `_operator.token_hashes` finds nothing for this
        // hash and denies.
        let tmp = tempfile::tempdir().unwrap();
        let operator_token = "operator-tok";
        let state = test_state(bootstrap_config(&tmp, operator_token), operator_token);

        let response = handle_create_profile(
            State(state.clone()),
            bearer_headers(operator_token),
            Json(ProfileBody {
                id: "ops".into(),
                allowed_agents: vec![],
                admin: true,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let result = require_admin(&state, &bearer_headers("never-paired-token")).await;
        assert!(
            result.is_err(),
            "an arbitrary bearer with no config binding at all must not ride the bootstrap rescue to admin"
        );
    }

    #[tokio::test]
    async fn bootstrap_rescue_denies_a_differently_paired_non_admin_principal() {
        // The CRITICAL case: Bob is genuinely paired (`gateway.paired_tokens`)
        // AND already explicitly configured as a non-admin "guest"
        // principal -- exactly what an operator does after bootstrapping:
        // pair a second device, bind it to a restricted role. `_operator`
        // already exists as admin (as if an earlier bootstrap already ran).
        // An earlier, flawed version of the rescue checked only "is this
        // bearer paired at all" (`require_auth`), which Bob's token would
        // have passed, letting him self-promote to admin. The fixed rescue
        // must deny him: his OWN hash resolves to "guest", not `_operator`.
        let tmp = tempfile::tempdir().unwrap();
        let operator_token = "operator-tok";
        let bob_token = "bob-tok";
        let mut authz = non_admin_authz(bob_token, "guest");
        authz.principals.push(PrincipalRecord {
            id: zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID.to_string(),
            allowed_agents: vec![],
            device_ids: vec![],
            token_hashes: vec![ConfigPairingGuard::token_hash(operator_token)],
            profiles: vec![zeroclaw_config::authz::OPERATOR_ADMIN_PROFILE_ID.to_string()],
        });
        authz.profiles.push(PermissionProfile {
            id: zeroclaw_config::authz::OPERATOR_ADMIN_PROFILE_ID.to_string(),
            allowed_agents: vec![],
            admin: true,
        });
        let mut cfg = temp_config(&tmp, authz);
        cfg.gateway.paired_tokens = vec![
            ConfigPairingGuard::token_hash(operator_token),
            ConfigPairingGuard::token_hash(bob_token),
        ];
        let state = test_state(cfg, operator_token);

        let result = require_admin(&state, &bearer_headers(bob_token)).await;
        let (status, _) = result.expect_err(
            "a paired principal explicitly bound to a non-admin role must never \
             self-promote via the bootstrap rescue, even while an operator-admin \
             principal already exists",
        );
        assert_eq!(status, StatusCode::FORBIDDEN);

        // The operator's own path is unaffected.
        assert!(
            require_admin(&state, &bearer_headers(operator_token))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn bootstrap_seed_scopes_admin_to_the_configuring_caller_only() {
        // Two devices were paired BEFORE authz existed (legacy
        // shared-operator state): the operator's and an unrelated second
        // device's. Only the operator actually performs the bootstrap
        // (creates the first profile). An earlier, flawed seed dumped ALL
        // of `gateway.paired_tokens` into `_operator`, silently promoting
        // the second device too. The fixed seed must bind ONLY the caller's
        // own hash.
        let tmp = tempfile::tempdir().unwrap();
        let operator_token = "operator-tok";
        let other_paired_token = "other-device-tok";
        let mut cfg = bootstrap_config(&tmp, operator_token);
        cfg.gateway
            .paired_tokens
            .push(ConfigPairingGuard::token_hash(other_paired_token));
        let state = test_state(cfg, operator_token);

        let response = handle_create_profile(
            State(state.clone()),
            bearer_headers(operator_token),
            Json(ProfileBody {
                id: "ops".into(),
                allowed_agents: vec![],
                admin: true,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let cfg = state.config.read().clone();
        let op = cfg
            .authz
            .by_id(zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID)
            .expect("operator principal seeded");
        assert_eq!(
            op.token_hashes,
            vec![ConfigPairingGuard::token_hash(operator_token)],
            "only the configuring caller's own hash must be seeded, never every paired token"
        );

        // The other, uninvolved paired device gets nothing.
        assert!(
            require_admin(&state, &bearer_headers(other_paired_token))
                .await
                .is_err(),
            "a merely-paired device that never configured anything must not become admin"
        );
    }

    #[tokio::test]
    async fn operator_bootstrap_seed_is_idempotent_across_repeated_config_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let operator_token = "operator-tok";
        let state = test_state(bootstrap_config(&tmp, operator_token), operator_token);

        for i in 0..2 {
            let response = handle_create_profile(
                State(state.clone()),
                bearer_headers(operator_token),
                Json(ProfileBody {
                    id: format!("profile-{i}"),
                    allowed_agents: vec![],
                    admin: false,
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert!(
                require_admin(&state, &bearer_headers(operator_token))
                    .await
                    .is_ok(),
                "operator must keep admin access across repeated config writes"
            );
        }

        let final_cfg = state.config.read().clone();
        assert_eq!(
            final_cfg
                .authz
                .principals
                .iter()
                .filter(|p| p.id == zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID)
                .count(),
            1,
            "the seed must not duplicate the operator principal across writes"
        );
        assert_eq!(
            final_cfg
                .authz
                .by_id(zeroclaw_config::authz::OPERATOR_PRINCIPAL_ID)
                .unwrap()
                .token_hashes
                .len(),
            1,
            "the bootstrap hash must not be duplicated either"
        );
    }

    // ── Profiles CRUD ───────────────────────────────────────────────

    #[tokio::test]
    async fn create_profile_rejects_duplicate_id() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        let body = || ProfileBody {
            id: "ops".into(), // already exists in admin_authz's fixture
            allowed_agents: vec![],
            admin: true,
        };
        let response = handle_create_profile(
            State(state.clone()),
            bearer_headers("bob-tok"),
            Json(body()),
        )
        .await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["code"], "conflict");
    }

    #[tokio::test]
    async fn update_profile_is_idempotent_upsert() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        for _ in 0..2 {
            let response = handle_update_profile(
                State(state.clone()),
                bearer_headers("bob-tok"),
                Json(ProfileBody {
                    id: "crm".into(),
                    allowed_agents: vec!["crm-bot".into()],
                    admin: false,
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        let cfg = state.config.read().clone();
        assert_eq!(
            cfg.authz.profiles.iter().filter(|p| p.id == "crm").count(),
            1
        );
        assert_eq!(
            cfg.authz
                .profiles
                .iter()
                .find(|p| p.id == "crm")
                .unwrap()
                .allowed_agents,
            vec!["crm-bot".to_string()]
        );
    }

    #[tokio::test]
    async fn delete_profile_leaves_a_dangling_ref_that_grants_nothing() {
        // bob is bound ONLY to "ops" (admin:true). Deleting "ops" must NOT
        // scrub bob's `profiles` list -- the ref stays, dangling -- and
        // once dangling it must resolve to zero grants, never "admin"/"*".
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");
        assert!(state.config.read().authz.is_admin("bob"));

        let response = handle_delete_profile(
            State(state.clone()),
            bearer_headers("bob-tok"),
            Query(ProfileIdQuery { id: "ops".into() }),
        )
        .await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["affected_principals"], serde_json::json!(["bob"]));

        let cfg = state.config.read().clone();
        // Dangling ref preserved, not scrubbed.
        assert_eq!(
            cfg.authz
                .principals
                .iter()
                .find(|p| p.id == "bob")
                .unwrap()
                .profiles,
            vec!["ops".to_string()]
        );
        // But it now resolves to nothing -- fail-closed, not "still admin".
        assert!(!cfg.authz.is_admin("bob"));
        assert!(cfg.authz.effective_agents("bob").is_empty());
    }

    #[tokio::test]
    async fn delete_profile_404s_for_unknown_id() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        let response = handle_delete_profile(
            State(state),
            bearer_headers("bob-tok"),
            Query(ProfileIdQuery { id: "ghost".into() }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ── Principal <-> profile binding ────────────────────────────────

    #[tokio::test]
    async fn bind_profile_to_principal_adds_it_once() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        for _ in 0..2 {
            let response = handle_bind_principal_profile(
                State(state.clone()),
                bearer_headers("bob-tok"),
                Path("bob".to_string()),
                Json(BindProfileBody {
                    profile_id: "crm".into(),
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        let cfg = state.config.read().clone();
        let bob = cfg.authz.principals.iter().find(|p| p.id == "bob").unwrap();
        assert_eq!(
            bob.profiles.iter().filter(|p| p.as_str() == "crm").count(),
            1,
            "re-binding the same profile_id must be idempotent, not duplicate"
        );
    }

    #[tokio::test]
    async fn bind_profile_404s_for_unknown_principal() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        let response = handle_bind_principal_profile(
            State(state),
            bearer_headers("bob-tok"),
            Path("ghost".to_string()),
            Json(BindProfileBody {
                profile_id: "crm".into(),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unbind_profile_removes_it_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let authz = admin_authz("bob-tok", "bob");
        let state = test_state(temp_config(&tmp, authz), "operator-tok");

        for _ in 0..2 {
            let response = handle_unbind_principal_profile(
                State(state.clone()),
                bearer_headers("bob-tok"),
                Path("bob".to_string()),
                Query(UnbindProfileQuery {
                    profile_id: "ops".into(),
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        let cfg = state.config.read().clone();
        let bob = cfg.authz.principals.iter().find(|p| p.id == "bob").unwrap();
        assert!(bob.profiles.is_empty());
    }

    // ── Agent alias picker ────────────────────────────────────────────

    #[tokio::test]
    async fn list_agents_returns_configured_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = temp_config(&tmp, AuthzConfig::default());
        cfg.create_map_key("agents", "crm-bot").unwrap();
        let state = test_state(cfg, "operator-tok");

        let response = handle_list_agents(State(state), bearer_headers("operator-tok")).await;
        let (status, json) = response_json(response).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["agents"], serde_json::json!(["crm-bot"]));
    }
}
