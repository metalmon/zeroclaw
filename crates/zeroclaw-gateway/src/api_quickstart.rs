//! HTTP routes for the Quickstart flow.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use zeroclaw_config::presets::BuilderSubmission;
use zeroclaw_runtime::quickstart::{
    AppliedAgent, QuickstartError, QuickstartStep, Surface, apply_with_surface, record_dismissed,
    validate_only_with_surface,
};

use super::AppState;
use super::api::require_auth;
use super::api_authz::require_admin;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidateResult {
    Ok,
    Errors { errors: Vec<QuickstartError> },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplyResult {
    Applied {
        agent: AppliedAgent,
        daemon_restarted: bool,
    },
    Errors {
        errors: Vec<QuickstartError>,
    },
}

pub async fn handle_state(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    let cfg = state.config.read().clone();
    let body = zeroclaw_runtime::quickstart::snapshot_state(&cfg);
    (StatusCode::OK, Json(body)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct FieldsRequest {
    pub section: zeroclaw_runtime::quickstart::FieldSection,
    pub type_key: String,
}

#[derive(Debug, Serialize)]
pub struct FieldsResult {
    pub fields: Vec<zeroclaw_runtime::quickstart::FieldDescriptor>,
}

pub async fn handle_fields(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FieldsRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    let body = FieldsResult {
        fields: zeroclaw_runtime::quickstart::field_shape(req.section, &req.type_key),
    };
    (StatusCode::OK, Json(body)).into_response()
}

pub async fn handle_validate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(submission): Json<BuilderSubmission>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    let cfg = state.config.read().clone();
    let body = match validate_only_with_surface(&submission, &cfg, Surface::Web) {
        Ok(()) => ValidateResult::Ok,
        Err(errors) => ValidateResult::Errors { errors },
    };
    (StatusCode::OK, Json(body)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct DismissRequest {
    pub run_id: String,
    pub surface: Surface,
    /// Furthest step the user reached. `None` = didn't progress past
    /// the first selector.
    #[serde(default)]
    pub last_step: Option<QuickstartStep>,
}

pub async fn handle_dismiss(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DismissRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    record_dismissed(&req.run_id, req.surface, req.last_step);
    (StatusCode::NO_CONTENT, ()).into_response()
}

pub async fn handle_apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(submission): Json<BuilderSubmission>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }
    if let Err(e) = require_admin(&state, &headers).await {
        return e.into_response();
    }
    // Held through the swap below (and across `apply_with_surface`'s own
    // save, which runs while this guard is held) so a concurrent config
    // writer can't land between this read and the swap.
    let _cfg_guard = std::sync::Arc::clone(&state.config_write_lock)
        .lock_owned()
        .await;
    let mut working = state.config.read().clone();
    let result = apply_with_surface(submission, &mut working, Surface::Web).await;
    let body = match result {
        Ok(agent) => {
            *state.config.write() = working;
            state
                .pending_reload
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let reload_signalled = signal_daemon_reload(&state);
            ApplyResult::Applied {
                agent,
                daemon_restarted: reload_signalled,
            }
        }
        Err(errors) => ApplyResult::Errors { errors },
    };
    (StatusCode::OK, Json(body)).into_response()
}

fn signal_daemon_reload(state: &AppState) -> bool {
    let Some(reload_tx) = state.reload_tx.clone() else {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({
                    "reason": "no_supervisor",
                })),
            "quickstart: daemon reload not available (standalone gateway)"
        );
        return false;
    };
    ::zeroclaw_log::record!(
        INFO,
        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Start),
        "quickstart: daemon reload signalled"
    );
    let shutdown_tx = state.shutdown_tx.clone();
    state
        .pending_reload
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let started = std::time::Instant::now();
    zeroclaw_spawn::spawn!(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let _ = shutdown_tx.send(true);
        let _ = reload_tx.send(true);
        ::zeroclaw_log::record!(
            INFO,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Complete)
                .with_outcome(::zeroclaw_log::EventOutcome::Success)
                .with_attrs(::serde_json::json!({
                    "elapsed_ms": started.elapsed().as_millis() as u64,
                })),
            "quickstart: daemon reload dispatched"
        );
    });
    true
}

// Per-family alias collection lives in
// `zeroclaw_runtime::quickstart::snapshot_state` so both transports
// share one implementation.

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, StatusCode, header};
    use zeroclaw_config::authz::{AuthzConfig, PermissionProfile, PrincipalRecord};
    use zeroclaw_config::presets::{AgentIdentity, SelectorChoice};

    fn quickstart_test_state(
        config: zeroclaw_config::schema::Config,
        require_pairing: bool,
        paired_tokens: &[String],
    ) -> AppState {
        let memory: std::sync::Arc<dyn zeroclaw_api::memory_traits::Memory> =
            std::sync::Arc::new(zeroclaw_memory::NoneMemory::new("none"));
        let authz = config.authz.clone();
        AppState {
            config: std::sync::Arc::new(parking_lot::RwLock::new(config)),
            config_write_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            model_provider: std::sync::Arc::new(crate::UnconfiguredModelProvider),
            model: "test-model".to_string(),
            temperature: None,
            mem: memory.clone(),
            memory_strategy: std::sync::Arc::new(
                zeroclaw_runtime::agent::memory_strategy::DefaultMemoryStrategy::with_config(
                    memory,
                    zeroclaw_config::schema::MemoryConfig::default(),
                    std::path::PathBuf::new(),
                ),
            ),
            auto_save: false,
            pairing: std::sync::Arc::new(zeroclaw_runtime::security::pairing::PairingGuard::new(
                require_pairing,
                paired_tokens,
            )),
            trust_forwarded_headers: false,
            rate_limiter: std::sync::Arc::new(crate::GatewayRateLimiter::new(100, 100, 100)),
            auth_limiter: std::sync::Arc::new(crate::auth_rate_limit::AuthRateLimiter::new()),
            idempotency_store: std::sync::Arc::new(crate::IdempotencyStore::new(
                std::time::Duration::from_secs(300),
                1000,
            )),
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
            observer: std::sync::Arc::new(zeroclaw_runtime::observability::NoopObserver),
            tools_registry: std::sync::Arc::new(Vec::new()),
            tools_registry_by_agent: std::sync::Arc::new(std::collections::HashMap::new()),
            cost_tracker: None,
            event_tx: tokio::sync::broadcast::channel(16).0,
            event_buffer: std::sync::Arc::new(crate::sse::EventBuffer::new(16)),
            shutdown_tx: tokio::sync::watch::channel(false).0,
            reload_tx: None,
            node_registry: std::sync::Arc::new(crate::nodes::NodeRegistry::new(16)),
            mdns_peer_registry: crate::nodes::mdns::MdnsPeerRegistry::default(),
            path_prefix: String::new(),
            web_dist_dir: None,
            session_backend: None,
            session_queue: std::sync::Arc::new(crate::session_queue::SessionActorQueue::new(
                8, 30, 600,
            )),
            device_registry: None,
            pending_pairings: None,
            canvas_store: zeroclaw_runtime::tools::CanvasStore::new(),
            #[cfg(feature = "webauthn")]
            webauthn: None,
            cancel_tokens: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            pending_reload: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

    fn minimal_submission(agent_name: &str) -> BuilderSubmission {
        BuilderSubmission {
            model_provider: SelectorChoice::Existing("dummy".to_string()),
            risk_profile: SelectorChoice::Existing("dummy".to_string()),
            runtime_profile: SelectorChoice::Existing("dummy".to_string()),
            memory: SelectorChoice::Existing("dummy".to_string()),
            channels: vec![],
            peer_groups: vec![],
            agent: AgentIdentity {
                name: agent_name.to_string(),
                system_prompt: String::new(),
                personality_file: None,
                personality_files: vec![],
            },
        }
    }

    #[tokio::test]
    async fn apply_denies_a_non_admin_paired_principal() {
        // Quickstart apply swaps live config (creates the agent, provider,
        // channels, ...) behind only the paired-guard today -- F4's
        // admin-gate must reject a paired-but-non-admin principal here too,
        // the same as every other config-write surface.
        let tmp = tempfile::tempdir().expect("tempdir");
        let authz = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".to_string(),
                allowed_agents: vec![],
                device_ids: vec![],
                token_hashes: vec![zeroclaw_config::pairing::PairingGuard::token_hash(
                    "alice-tok",
                )],
                profiles: vec!["crm".to_string()],
            }],
            profiles: vec![PermissionProfile {
                id: "crm".to_string(),
                allowed_agents: vec!["crm-bot".to_string()],
                admin: false,
            }],
        };
        let cfg = zeroclaw_config::schema::Config {
            config_path: tmp.path().join("config.toml"),
            authz,
            ..Default::default()
        };
        let state = quickstart_test_state(cfg, true, &["alice-tok".to_string()]);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str("Bearer alice-tok").unwrap(),
        );

        let response = handle_apply(
            State(state.clone()),
            headers,
            Json(minimal_submission("new-agent")),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(
            !state.config.read().agents.contains_key("new-agent"),
            "a rejected quickstart apply must not create the agent"
        );
    }

    #[tokio::test]
    async fn apply_allows_an_unpaired_caller_while_pairing_is_disabled() {
        // Sanity check on the fixture itself: with pairing disabled and
        // authz unconfigured, the lockout fallback must not block a bare
        // request from reaching past the gate (it may still fail later in
        // `apply_with_surface` on the dummy aliases -- this only asserts
        // the gate itself doesn't 401/403 it).
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = zeroclaw_config::schema::Config {
            config_path: tmp.path().join("config.toml"),
            ..Default::default()
        };
        let state = quickstart_test_state(cfg, false, &[]);

        let response = handle_apply(
            State(state),
            HeaderMap::new(),
            Json(minimal_submission("new-agent")),
        )
        .await
        .into_response();

        assert_ne!(response.status(), StatusCode::FORBIDDEN);
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
