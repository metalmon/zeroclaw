//! ACP-over-WebSocket gateway endpoint.
//!
//! Fork-local addition on top of upstream's `/acp` transport: a pre-auth
//! gate (pairing-over-ACP, `zeroclaw/pair`/`volt/pair`) that resolves a
//! bearer token to a distinct, grant-scoped [`Principal`] via the
//! dedicated `"pairing"` auth provider (see
//! `zeroclaw_runtime::security::pairing_auth_provider`), instead of the bare
//! paired/not-paired check upstream's `handle_ws_acp` used alone. Upstream's
//! own doc comment at the session-persistence call site
//! (`zeroclaw-channels`' `acp_server.rs`) names per-agent principal binding
//! on this transport as its own unbuilt future stage (RFC 7141 F2) — this
//! module fills exactly that gap, additively.

use super::AppState;
use axum::{
    extract::{
        ConnectInfo, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use zeroclaw_api::grants::ResolvedGrants;
use zeroclaw_api::jsonrpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcResponse, error_codes};
use zeroclaw_api::principal::{AuthOutcome, Principal};
use zeroclaw_channels::orchestrator::acp_server::{AcpServer, AcpServerConfig};
use zeroclaw_infra::acp_session_store::AcpSessionStore;
use zeroclaw_runtime::security::auth_provider::{Credential, ProviderRegistry};
use zeroclaw_runtime::security::pairing_auth_provider::PairingAuthProvider;
use zeroclaw_runtime::security::principal_resolver::PrincipalResolver;

const ACP_WS_PROTOCOL: &str = "zeroclaw.acp.v1";

/// How long an unauthenticated `/acp` connection may sit in pre-auth mode
/// (only `zeroclaw/pair`/`volt/pair` accepted) before the gateway drops it.
/// Bounds the resource an anonymous inbound connection can hold open
/// without ever pairing.
const PRE_AUTH_HANDSHAKE_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Deserialize)]
pub struct AcpQuery {
    token: Option<String>,
    /// Connection-scoped default agent alias. Used when `session/new` omits
    /// `agentAlias`, so spec-vanilla one-agent-per-endpoint ACP clients can
    /// address each configured agent via its own URL. Validated exactly like
    /// an explicit `agentAlias` at `session/new`; ignored by session restore.
    agent: Option<String>,
}

pub async fn handle_ws_acp(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Query(params): Query<AcpQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let token = extract_ws_token(&headers, params.token.as_deref()).map(str::to_string);

    // Resolve the presented bearer up front (pure reads only — no
    // binding-store writes happen on this path) so its outcome feeds BOTH
    // the pre-auth/authenticated gate below and, on the authenticated
    // branch, the principal+grants actually bound to the socket.
    let resolved = resolve_principal(&state, token.as_deref()).await;

    // Pairing-over-ACP (fork-local): when pairing is required and the
    // presented token is absent or not (yet) paired, do NOT 401 — upgrade
    // the connection in pre-auth mode instead, UNLESS the credential
    // already resolved to a DISTINCT authenticated principal. A pre-auth
    // connection may call only `zeroclaw/pair`/`volt/pair`; on a successful
    // pair it resolves its principal and proceeds on the SAME socket (see
    // `handle_socket`). A deployment with `require_pairing = false`, or a
    // connection presenting an already-paired token, takes the
    // authenticated path exactly as before.
    let authenticated = connection_is_authenticated(
        state.pairing.require_pairing(),
        token
            .as_deref()
            .is_some_and(|t| state.pairing.is_authenticated(t)),
        resolved.as_ref().map(|(principal, _)| principal),
    );

    let ws = if headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|protos| protos.split(',').any(|p| p.trim() == ACP_WS_PROTOCOL))
    {
        ws.protocols([ACP_WS_PROTOCOL])
    } else {
        ws
    };

    if !authenticated {
        // Pre-auth: no principal to resolve yet, no 401 — the connection
        // itself is allowed; only its method set is restricted, enforced in
        // `handle_socket`. `client_id` buckets `PairingGuard`'s brute-force
        // lockout the same way the REST `/pair` front door keys it: by peer
        // identity.
        let client_id = peer_addr.to_string();
        return ws
            .on_upgrade(move |socket| {
                handle_socket(socket, state, params.agent, client_id, ConnAuth::PreAuth)
            })
            .into_response();
    }

    // The authenticated subject for this connection — reusing `resolved`
    // from the up-front resolution above (no second registry round-trip).
    // Fail-closed once authz is enforced; otherwise fall back to the shared
    // operator so the legacy single-operator path (including no-pairing
    // deployments) is unchanged. A presented-but-Denied bearer is still
    // rejected with a hard 401 here.
    let authz_enforced = state.config.read().authz.is_enforced();
    let (principal, grants) = match resolved {
        Some(pg) => pg,
        None if !authz_enforced => (Principal::shared_operator(), ResolvedGrants::all()),
        None => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized - no principal is entitled for the presented credential",
            )
                .into_response();
        }
    };

    let client_id = peer_addr.to_string();
    ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            params.agent,
            client_id,
            ConnAuth::Authenticated(principal, grants),
        )
    })
    .into_response()
}

/// The connection-gate decision: should this `/acp` connection skip
/// pre-auth pairing and proceed straight to `handle_socket` as
/// authenticated? `true` when ANY of:
/// 1. pairing is not required at all (`!require_pairing`, legacy, unchanged),
/// 2. the presented token is already a valid *paired* token
///    (`token_is_paired`, the existing `PairingGuard` path), or
/// 3. the credential already resolved to a DISTINCT authenticated principal
///    (`resolved.is_authenticated()`) — a bearer bound via
///    `[[authz.principals]]`/[`zeroclaw_config::authz::TokenBindingStore`]
///    under enforced authz.
///
/// Condition 3 can never be satisfied by [`Principal::shared_operator`]
/// (`is_authenticated() == false`), so a bare paired-but-unbound token can
/// NEVER bypass `require_pairing` when authz is NOT enforced — only a
/// config-backed *distinct* principal (i.e. authz IS enforced and the token
/// actually maps to a configured principal) skips pairing.
fn connection_is_authenticated(
    require_pairing: bool,
    token_is_paired: bool,
    resolved: Option<&Principal>,
) -> bool {
    !require_pairing || token_is_paired || resolved.is_some_and(Principal::is_authenticated)
}

/// The connection's auth state at the moment its socket is handed to
/// `handle_socket`. `PreAuth` is resolved into `Authenticated` in-place, on
/// the same connection, by a successful `zeroclaw/pair`/`volt/pair` — see
/// `run_pre_auth`.
enum ConnAuth {
    Authenticated(Principal, ResolvedGrants),
    PreAuth,
}

/// Build the ACP-dedicated connection auth registry: just the
/// [`PairingAuthProvider`], registered under the name `"pairing"` —
/// distinct from (and never selected instead of) upstream's own `"native"`
/// provider used by the daemon's other RPC transports. Kept as its own
/// registry (rather than joining the daemon's main one) to minimize blast
/// radius: ACP's bearer-to-principal resolution stays self-contained. Built
/// fresh per connection from the live config snapshot, so a config edit
/// (new/removed principal, rebound profile) applies with no reload.
#[must_use]
fn build_provider_registry(
    authz: zeroclaw_config::authz::AuthzConfig,
    bindings: Arc<zeroclaw_config::authz::TokenBindingStore>,
) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    if let Err(e) = registry.register(Arc::new(PairingAuthProvider::new(authz, bindings))) {
        // Unreachable in practice: "pairing" is a fixed, fork-owned name
        // registered exactly once into a brand-new registry. Fail safe
        // (empty registry => default-deny) rather than panic if it ever is.
        ::zeroclaw_log::record!(
            ERROR,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": e.to_string()})),
            "failed to register the ACP pairing auth provider"
        );
    }
    registry
}

/// Resolve a presented bearer to a [`Principal`] + its [`ResolvedGrants`]
/// via the ACP-dedicated registry (`"pairing"` provider) and the shared
/// [`PrincipalResolver`]. `None` means the credential was denied (or
/// absent); the caller decides whether that is a hard reject (authz
/// enforced) or the shared-operator fallback (legacy).
async fn resolve_principal(
    state: &AppState,
    token: Option<&str>,
) -> Option<(Principal, ResolvedGrants)> {
    let token = token.filter(|t| !t.is_empty()).map(str::to_string);
    let credential = match token {
        Some(token) => Credential::Bearer(token),
        None => Credential::None,
    };
    let config = state.config.read().clone();
    let registry = build_provider_registry(config.authz.clone(), Arc::clone(&state.token_bindings));
    let outcome = registry.resolve_named("pairing", &credential).await;
    let identity = match outcome {
        AuthOutcome::Verified(identity) => identity,
        AuthOutcome::Denied { .. } => return None,
    };
    let resolver = match PrincipalResolver::from_config(&config) {
        Ok(resolver) => resolver,
        Err(e) => {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"error": e.to_string()})),
                "ACP pairing: failed to compile the authorization policy; denying"
            );
            return None;
        }
    };
    match resolver.resolve(&identity) {
        Ok(resolved) => Some((resolved.principal, resolved.grants)),
        Err(_deny) => None,
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    default_agent: Option<String>,
    client_id: String,
    auth: ConnAuth,
) {
    let (mut sender, mut receiver) = socket.split();

    let (principal, grants) = match auth {
        ConnAuth::Authenticated(principal, grants) => (principal, grants),
        ConnAuth::PreAuth => {
            match run_pre_auth(&mut sender, &mut receiver, &state, &client_id).await {
                Some(pg) => pg,
                // Idle timeout, socket closed, or write failure before a
                // successful pair: nothing more to do. No `AcpServer` or
                // session was ever constructed for this connection.
                None => return,
            }
        }
    };

    let (input_tx, input_rx) = mpsc::channel::<String>(256);
    let (output_tx, mut output_rx) = mpsc::channel::<String>(256);

    let config = state.config.read().clone();
    let acp_config = AcpServerConfig {
        max_sessions: config.acp.max_sessions,
        session_timeout_secs: config.acp.session_timeout_secs,
    };
    let store = AcpSessionStore::new(&config.data_dir)
        .map(Arc::new)
        .inspect_err(|e| {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"error": e.to_string()})),
                "Failed to open ACP session store"
            );
        })
        .ok();
    let canvas_store = state.canvas_store.clone();
    let server = if let Some(store) = store {
        Arc::new(
            AcpServer::new_with_live_config_and_writer_and_store(
                Arc::clone(&state.config),
                state.agent_lifecycle.clone(),
                acp_config,
                output_tx,
                store,
            )
            .with_canvas_store(canvas_store)
            .with_sop_engine(state.sop_engine.clone(), state.sop_audit.clone())
            .with_task_supervisor(state.task_supervisor.clone())
            .with_mcp_pool(state.mcp_pool.clone())
            .with_connection_default_agent(default_agent)
            .with_principal(principal)
            .with_grants(grants),
        )
    } else {
        Arc::new(
            AcpServer::new_with_live_config_and_writer(
                Arc::clone(&state.config),
                state.agent_lifecycle.clone(),
                acp_config,
                output_tx,
            )
            .with_canvas_store(canvas_store)
            .with_sop_engine(state.sop_engine.clone(), state.sop_audit.clone())
            .with_task_supervisor(state.task_supervisor.clone())
            .with_mcp_pool(state.mcp_pool.clone())
            .with_connection_default_agent(default_agent)
            .with_principal(principal)
            .with_grants(grants),
        )
    };

    let server_task = zeroclaw_spawn::spawn!(Arc::clone(&server).run_messages(input_rx));

    let output_task = zeroclaw_spawn::spawn!(async move {
        while let Some(line) = output_rx.recv().await {
            if sender.send(Message::Text(line.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(message) = receiver.next().await {
        match message {
            Ok(Message::Text(text)) => {
                if input_tx.send(text.to_string()).await.is_err() {
                    break;
                }
            }
            Ok(Message::Binary(bytes)) => match String::from_utf8(bytes.to_vec()) {
                Ok(text) => {
                    if input_tx.send(text).await.is_err() {
                        break;
                    }
                }
                Err(e) => ::zeroclaw_log::record!(
                    WARN,
                    ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                        .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                        .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                    "ACP WebSocket received non-UTF-8 binary frame"
                ),
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Connection reset without closing handshake")
                    || msg.contains("Connection closed normally")
                {
                    ::zeroclaw_log::record!(
                        DEBUG,
                        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note),
                        "ACP WebSocket closed without handshake"
                    );
                } else {
                    ::zeroclaw_log::record!(
                        WARN,
                        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                            .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                            .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                        "ACP WebSocket receive error"
                    );
                }
                break;
            }
        }
    }

    drop(input_tx);

    if let Err(e) = server_task.await {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
            "ACP WebSocket server task panicked"
        );
    }
    output_task.abort();
    ::zeroclaw_log::record!(
        DEBUG,
        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note),
        "ACP WebSocket disconnected"
    );
}

/// Pre-auth frame loop for an unauthenticated `/acp` connection. Reads
/// JSON-RPC frames directly off the socket (no `AcpServer`, no session —
/// none is constructed until a principal is resolved). The only accepted
/// methods are `zeroclaw/pair` and its `volt/pair` alias; every other
/// method gets an `AuthRequired` error and the loop keeps waiting. Returns
/// the resolved `(Principal, ResolvedGrants)` on a successful pair (the
/// caller then proceeds on the SAME socket, reusing `sender` and
/// `receiver`), or `None` if the socket closes, errors, a write fails, or
/// the handshake timeout elapses first.
async fn run_pre_auth(
    sender: &mut SplitSink<WebSocket, Message>,
    receiver: &mut SplitStream<WebSocket>,
    state: &AppState,
    client_id: &str,
) -> Option<(Principal, ResolvedGrants)> {
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(PRE_AUTH_HANDSHAKE_TIMEOUT_SECS);

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let message = match tokio::time::timeout(remaining, receiver.next()).await {
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(_)) | None) | Err(_) => return None,
        };

        let text = match message {
            Message::Text(text) => text.to_string(),
            Message::Binary(bytes) => match String::from_utf8(bytes.to_vec()) {
                Ok(text) => text,
                Err(_) => continue,
            },
            Message::Close(_) => return None,
            Message::Ping(_) | Message::Pong(_) => continue,
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(e) => {
                if !send_pre_auth_error(
                    sender,
                    Value::Null,
                    error_codes::PARSE_ERROR,
                    &format!("Parse error: {e}"),
                )
                .await
                {
                    return None;
                }
                continue;
            }
        };
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        let method = value.get("method").and_then(Value::as_str).unwrap_or("");

        // `volt/pair` is an accepted alias for `zeroclaw/pair` (rebrand):
        // both names redeem the SAME pairing code through the SAME
        // `PairingGuard`/`TokenBindingStore` path below.
        if method != "zeroclaw/pair" && method != "volt/pair" {
            if !send_pre_auth_error_with_reason(
                sender,
                id,
                error_codes::AUTH_REQUIRED,
                "pair first via zeroclaw/pair",
                Some("pair_first"),
            )
            .await
            {
                return None;
            }
            continue;
        }

        let code = value
            .get("params")
            .and_then(|params| params.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("");

        match state.pairing.try_pair(code, client_id).await {
            Ok(Some(token)) => {
                let authz_enforced = state.config.read().authz.is_enforced();
                let principal_and_grants = match resolve_principal(state, Some(&token)).await {
                    Some(pg) => pg,
                    None if !authz_enforced => {
                        (Principal::shared_operator(), ResolvedGrants::all())
                    }
                    None => {
                        // Paired, but the issued token is not entitled under
                        // enforced authz (e.g. not bound to any configured
                        // principal). Stay pre-auth rather than proceeding
                        // with no principal.
                        if !send_pre_auth_error_with_reason(
                            sender,
                            id,
                            error_codes::AUTH_REQUIRED,
                            "pairing succeeded but no principal is entitled for the issued token",
                            Some("paired_not_entitled"),
                        )
                        .await
                        {
                            return None;
                        }
                        continue;
                    }
                };

                if !send_pre_auth_result(sender, id, serde_json::json!({ "token": token })).await {
                    return None;
                }
                return Some(principal_and_grants);
            }
            Ok(None) => {
                if !send_pre_auth_error_with_reason(
                    sender,
                    id,
                    error_codes::AUTH_REQUIRED,
                    "invalid pairing code",
                    Some("invalid_pair_code"),
                )
                .await
                {
                    return None;
                }
            }
            Err(retry_after) => {
                if !send_pre_auth_error(
                    sender,
                    id,
                    error_codes::AUTH_REQUIRED,
                    &format!("too many attempts, retry after {retry_after}s"),
                )
                .await
                {
                    return None;
                }
            }
        }
    }
}

/// Write a JSON-RPC success response directly to the WS sink, before any
/// `AcpServer`/`RpcOutbound` exists for this connection. Returns whether the
/// write succeeded.
async fn send_pre_auth_result(
    sender: &mut SplitSink<WebSocket, Message>,
    id: Value,
    result: Value,
) -> bool {
    send_pre_auth_response(sender, id, Some(result), None).await
}

/// Write a JSON-RPC error response directly to the WS sink. Returns whether
/// the write succeeded.
async fn send_pre_auth_error(
    sender: &mut SplitSink<WebSocket, Message>,
    id: Value,
    code: i32,
    message: &str,
) -> bool {
    send_pre_auth_error_with_reason(sender, id, code, message, None).await
}

/// Same as [`send_pre_auth_error`], but tags the frame with a stable,
/// machine-readable `data.reason` so panel clients can show a localized
/// headline without parsing the English `message` — which stays exactly as
/// written, appended as the human-readable / EN-fallback detail.
async fn send_pre_auth_error_with_reason(
    sender: &mut SplitSink<WebSocket, Message>,
    id: Value,
    code: i32,
    message: &str,
    reason: Option<&'static str>,
) -> bool {
    send_pre_auth_response(
        sender,
        id,
        None,
        Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: reason.map(|r| serde_json::json!({ "reason": r })),
        }),
    )
    .await
}

async fn send_pre_auth_response(
    sender: &mut SplitSink<WebSocket, Message>,
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
) -> bool {
    let response = JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        result,
        error,
        id,
    };
    match serde_json::to_string(&response) {
        Ok(json) => sender.send(Message::Text(json.into())).await.is_ok(),
        Err(_) => false,
    }
}

fn extract_ws_token<'a>(headers: &'a HeaderMap, query_token: Option<&'a str>) -> Option<&'a str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .or_else(|| {
            headers
                .get(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
                .and_then(|v| v.to_str().ok())
                .and_then(|protocols| {
                    protocols
                        .split(',')
                        .map(str::trim)
                        .find_map(|p| p.strip_prefix("bearer."))
                })
                .filter(|token| !token.is_empty())
        })
        .or_else(|| query_token.filter(|token| !token.is_empty()))
}

#[cfg(test)]
mod tests {
    //! Front-door proof for the gateway ACP surface.
    //!
    //! The `session/new` unit tests in `zeroclaw-channels` call
    //! `handle_session_new` directly; they prove the shared handler but cross
    //! neither user boundary. This test drives the *real* `/acp` WebSocket
    //! route: it boots the full gateway via `run_gateway` (so the request
    //! passes through the axum router, the pairing/subprotocol middleware in
    //! `handle_ws_acp`, the `on_upgrade` bridge, and `run_messages`), then
    //! connects a real WebSocket client and issues `session/new` with an
    //! omitted `cwd`. It asserts the returned `workspaceDir` is exactly the
    //! per-agent workspace — the behavior this PR introduces — rather than the
    //! daemon process CWD.

    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    /// On-disk-equivalent of the channels-crate `make_test_config`: a fake
    /// `anthropic.default` provider (model name only, no key — agent
    /// construction is offline) and one dispatchable agent `test-agent`, with
    /// `config_path` pinned so `agent_workspace_dir` resolves under the temp
    /// install root. Pairing is disabled so the WS client needs no token.
    fn front_door_config(install_root: &std::path::Path) -> zeroclaw_config::schema::Config {
        use zeroclaw_config::schema::{
            AliasedAgentConfig, AnthropicModelProviderConfig, Config, ModelProviderConfig,
            RiskProfileConfig, RuntimeProfileConfig,
        };

        let mut cfg = Config {
            data_dir: install_root.join("data"),
            config_path: install_root.join("config.toml"),
            ..Config::default()
        };
        cfg.gateway.require_pairing = false;
        cfg.providers.models.anthropic.insert(
            "default".to_string(),
            AnthropicModelProviderConfig {
                base: ModelProviderConfig {
                    model: Some("claude-haiku-4-5".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        cfg.risk_profiles
            .insert("default".to_string(), RiskProfileConfig::default());
        cfg.runtime_profiles
            .insert("default".to_string(), RuntimeProfileConfig::default());
        cfg.agents.insert(
            "test-agent".to_string(),
            AliasedAgentConfig {
                model_provider: "anthropic.default".into(),
                risk_profile: "default".into(),
                runtime_profile: "default".into(),
                ..Default::default()
            },
        );
        cfg
    }

    #[tokio::test]
    async fn acp_ws_front_door_omitted_cwd_uses_agent_workspace() {
        // `run_gateway` binds the process-global pricing config handle.
        let _pricing_binding = crate::PRICING_BINDING_TEST_LOCK.lock().await;
        let tmp = tempfile::tempdir().unwrap();
        let install_root = tmp.path();
        let cfg = front_door_config(install_root);
        std::fs::create_dir_all(&cfg.data_dir).unwrap();
        let expected_ws = cfg
            .agent_workspace_dir("test-agent")
            .to_string_lossy()
            .into_owned();

        // Reserve an ephemeral port, then hand it to the gateway.
        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        let (reload_tx, _reload_rx) = tokio::sync::watch::channel(false);
        let reload_controls = zeroclaw_runtime::daemon::GatewayReloadControls::standalone(
            shutdown_tx.clone(),
            reload_tx,
        );

        let server = zeroclaw_spawn::spawn!(crate::run_gateway(
            "127.0.0.1",
            port,
            cfg,
            None,
            Some(reload_controls),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ));

        // Wait until the gateway is accepting TCP connections.
        let addr = format!("127.0.0.1:{port}");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if tokio::net::TcpStream::connect(&addr).await.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("gateway should accept connections");

        let url = format!("ws://127.0.0.1:{port}/acp");
        let (mut ws, _resp) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WebSocket upgrade on /acp should succeed");

        ws.send(Message::Text(
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc":"2.0","id":2,"method":"session/new",
                "params":{"agentAlias":"test-agent"}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        let workspace_dir = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while let Some(msg) = ws.next().await {
                let text = match msg {
                    Ok(Message::Text(t)) => t.to_string(),
                    Ok(_) => continue,
                    Err(e) => panic!("WebSocket read error: {e}"),
                };
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if value.get("id").and_then(|i| i.as_i64()) == Some(2) {
                    if let Some(err) = value.get("error") {
                        panic!("session/new returned an error: {err}");
                    }
                    return value["result"]["workspaceDir"].as_str().map(String::from);
                }
            }
            None
        })
        .await
        .expect("session/new response should arrive before timeout");

        shutdown_tx.send(true).ok();
        server.abort();

        assert_eq!(
            workspace_dir.as_deref(),
            Some(expected_ws.as_str()),
            "omitted-cwd session/new over the real /acp WebSocket route must \
             return the per-agent workspace, not the daemon CWD"
        );
    }

    /// FROZEN: a bearer token that is already validly PAIRED (so the
    /// pre-auth pairing gate is satisfied — `connection_is_authenticated`'s
    /// condition 2) but, under enforced `[[authz.principals]]`, does not
    /// bind to any configured principal must be rejected with a hard `401`
    /// at the `/acp` WebSocket upgrade — never silently fall through to
    /// shared-operator, and never left to linger in pre-auth (it is
    /// already "authenticated" in the pairing sense, so pre-auth is the
    /// wrong state for it; only a 401 is correct).
    #[tokio::test]
    async fn acp_ws_paired_but_not_entitled_bearer_is_rejected_with_401() {
        let _pricing_binding = crate::PRICING_BINDING_TEST_LOCK.lock().await;
        let tmp = tempfile::tempdir().unwrap();
        let install_root = tmp.path();
        let mut cfg = front_door_config(install_root);
        cfg.gateway.require_pairing = true;
        cfg.gateway.paired_tokens = vec!["zc_paired_but_unbound".to_string()];
        // Authz IS enforced (one principal configured), but it is bound to a
        // DIFFERENT token than the one this connection presents.
        cfg.authz.principals.push(zeroclaw_config::authz::PrincipalRecord {
            id: "someone-else".to_string(),
            token_hashes: vec![zeroclaw_config::pairing::PairingGuard::token_hash(
                "zc_some_other_token",
            )],
            profiles: vec![],
        });
        std::fs::create_dir_all(&cfg.data_dir).unwrap();

        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        let (reload_tx, _reload_rx) = tokio::sync::watch::channel(false);
        let reload_controls = zeroclaw_runtime::daemon::GatewayReloadControls::standalone(
            shutdown_tx.clone(),
            reload_tx,
        );

        let server = zeroclaw_spawn::spawn!(crate::run_gateway(
            "127.0.0.1",
            port,
            cfg,
            None,
            Some(reload_controls),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ));

        let addr = format!("127.0.0.1:{port}");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if tokio::net::TcpStream::connect(&addr).await.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("gateway should accept connections");

        let url = format!("ws://127.0.0.1:{port}/acp");
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut request = url.into_client_request().unwrap();
        request.headers_mut().insert(
            axum::http::header::AUTHORIZATION,
            "Bearer zc_paired_but_unbound".parse().unwrap(),
        );
        let err = tokio_tungstenite::connect_async(request)
            .await
            .expect_err("a paired-but-not-entitled bearer must be rejected, not upgraded");

        shutdown_tx.send(true).ok();
        server.abort();

        match err {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected an HTTP 401 handshake rejection, got: {other:?}"),
        }
    }

    /// Characterization: a bearer bound (via the live
    /// [`zeroclaw_config::authz::TokenBindingStore`], pre-seeded here the
    /// same way a principal-tagged pairing redemption would write it) to a
    /// `[[authz.principals]]` entry resolves to a DISTINCT `Roster`
    /// principal — not `SharedOperator`'s unconditional access — visible
    /// through its live grant set: the connection can open a session for
    /// the ONE agent its bound profile grants, and is denied the OTHER
    /// configured agent with the FROZEN `agent_not_permitted` shape,
    /// proving it never silently fell back to shared-operator.
    #[tokio::test]
    async fn acp_ws_bound_bearer_resolves_to_a_distinct_entitled_principal() {
        use zeroclaw_config::schema::{
            AliasedAgentConfig, AnthropicModelProviderConfig, ModelProviderConfig,
            PermissionProfileConfig, RiskProfileConfig, RuntimeProfileConfig,
        };

        let _pricing_binding = crate::PRICING_BINDING_TEST_LOCK.lock().await;
        let tmp = tempfile::tempdir().unwrap();
        let install_root = tmp.path();
        let mut cfg = front_door_config(install_root);
        cfg.agents.insert(
            "other-agent".to_string(),
            AliasedAgentConfig {
                model_provider: "anthropic.default".into(),
                risk_profile: "default".into(),
                runtime_profile: "default".into(),
                ..Default::default()
            },
        );
        cfg.providers.models.anthropic.insert(
            "default".to_string(),
            AnthropicModelProviderConfig {
                base: ModelProviderConfig {
                    model: Some("claude-haiku-4-5".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        cfg.risk_profiles
            .insert("default".to_string(), RiskProfileConfig::default());
        cfg.runtime_profiles
            .insert("default".to_string(), RuntimeProfileConfig::default());
        cfg.permission_profiles.insert(
            "crm".to_string(),
            PermissionProfileConfig {
                allowed_agents: vec!["test-agent".to_string()],
                ..PermissionProfileConfig::default()
            },
        );
        cfg.gateway.require_pairing = true;
        let bound_token = "zc_bound_device_token".to_string();
        cfg.gateway.paired_tokens = vec![bound_token.clone()];
        cfg.authz
            .principals
            .push(zeroclaw_config::authz::PrincipalRecord {
                id: "bound-device".to_string(),
                token_hashes: vec![],
                profiles: vec!["crm".to_string()],
            });
        std::fs::create_dir_all(&cfg.data_dir).unwrap();
        // Pre-seed the live binding store exactly as a principal-tagged
        // pairing redemption would write it (`state.token_bindings.set`),
        // so this test exercises the SAME store the real gateway reads
        // from at `AppState` construction (`TokenBindingStore::new(&cfg.data_dir)`).
        zeroclaw_config::authz::TokenBindingStore::new(&cfg.data_dir)
            .set(
                zeroclaw_config::pairing::PairingGuard::token_hash(&bound_token),
                "bound-device".to_string(),
            )
            .expect("seed the token binding before the gateway starts");

        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        let (reload_tx, _reload_rx) = tokio::sync::watch::channel(false);
        let reload_controls = zeroclaw_runtime::daemon::GatewayReloadControls::standalone(
            shutdown_tx.clone(),
            reload_tx,
        );
        let server = zeroclaw_spawn::spawn!(crate::run_gateway(
            "127.0.0.1",
            port,
            cfg,
            None,
            Some(reload_controls),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ));

        let addr = format!("127.0.0.1:{port}");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if tokio::net::TcpStream::connect(&addr).await.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("gateway should accept connections");

        let connect_and_session_new = |agent_alias: &'static str| {
            let url = format!("ws://127.0.0.1:{port}/acp");
            let token = bound_token.clone();
            async move {
                use tokio_tungstenite::tungstenite::client::IntoClientRequest;
                let mut request = url.into_client_request().unwrap();
                request.headers_mut().insert(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token}").parse().unwrap(),
                );
                let (mut ws, _resp) = tokio_tungstenite::connect_async(request)
                    .await
                    .expect("a bound, already-paired bearer must upgrade authenticated");
                ws.send(Message::Text(
                    serde_json::json!({
                        "jsonrpc":"2.0","id":1,"method":"session/new",
                        "params":{"agentAlias": agent_alias, "mcpServers": []}
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    while let Some(msg) = ws.next().await {
                        let text = match msg {
                            Ok(Message::Text(t)) => t.to_string(),
                            Ok(_) => continue,
                            Err(e) => panic!("WebSocket read error: {e}"),
                        };
                        let value: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if value.get("id").and_then(|i| i.as_i64()) == Some(1) {
                            return value;
                        }
                    }
                    panic!("session/new response should arrive before timeout");
                })
                .await
                .expect("session/new must answer before the test timeout")
            }
        };

        let permitted = connect_and_session_new("test-agent").await;
        let denied = connect_and_session_new("other-agent").await;

        shutdown_tx.send(true).ok();
        server.abort();

        assert!(
            permitted.get("error").is_none(),
            "the bound principal's own agent (test-agent, via the \"crm\" \
             profile) must be permitted; got: {permitted}"
        );
        assert_eq!(
            denied["error"]["code"].as_i64(),
            Some(-32602),
            "got: {denied}"
        );
        assert_eq!(
            denied["error"]["data"]["reason"].as_str(),
            Some("agent_not_permitted"),
            "a principal entitled only to test-agent must be denied other-agent \
             with the FROZEN agent_not_permitted shape, not silently permitted \
             as shared-operator; got: {denied}"
        );
    }
}
