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
/// control plane before Task 4 seeds a bootstrap admin principal. The
/// instant any principal or profile is configured, this fallback stops
/// applying: an unresolved credential, a credential the auth provider
/// denies, or a credential that resolves to a principal not bound to any
/// `admin` profile are all a flat 403 from here on.
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
        Ok(())
    } else {
        Err(forbidden_error())
    }
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
    if let Err(msg) = working.create_map_key("authz.profiles", &body.id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::InternalError, msg).with_path("authz.profiles"),
        );
    }
    if let Err(e) = apply_profile_fields(&mut working, &body) {
        return error_response(e);
    }
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
    if let Err(msg) = working.create_map_key("authz.profiles", &body.id) {
        return error_response(
            ConfigApiError::new(ConfigApiCode::InternalError, msg).with_path("authz.profiles"),
        );
    }
    if let Err(e) = apply_profile_fields(&mut working, &body) {
        return error_response(e);
    }
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
