//! ACP-over-WebSocket gateway endpoint.

use super::{AppState, ClientDeviceId};
use axum::{
    Extension,
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
use zeroclaw_api::jsonrpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcResponse, error_codes};
use zeroclaw_api::principal::{AuthOutcome, Principal};
use zeroclaw_channels::orchestrator::acp_server::{AcpServer, AcpServerConfig};
use zeroclaw_config::authz::AuthzConfig;
use zeroclaw_infra::acp_session_store::AcpSessionStore;
use zeroclaw_runtime::security::auth_provider::{Credential, ProviderRegistry};
use zeroclaw_runtime::security::pairing_auth_provider::PairingAuthProvider;

const ACP_WS_PROTOCOL: &str = "zeroclaw.acp.v1";

/// How long an unauthenticated `/acp` connection may sit in pre-auth mode
/// (only `zeroclaw/pair` accepted) before the gateway drops it. Bounds the
/// resource an anonymous inbound connection can hold open without ever
/// pairing.
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
    // Present only on a connection whose TLS layer is mTLS AND presented a
    // client certificate (the public accept loop attaches this extension
    // per-connection from the peer cert's subject CN). Absent on the
    // loopback/private listener (no TLS) and on any TLS connection without a
    // client cert — `device_id` stays `None`, unchanged behavior.
    device_id_ext: Option<Extension<ClientDeviceId>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let token = extract_ws_token(&headers, params.token.as_deref()).map(str::to_string);
    let device_id = device_id_ext.map(|Extension(ClientDeviceId(id))| id);

    // Resolve the presented credential up front (pure reads only — no
    // binding-store writes happen on this path) so its outcome can feed
    // BOTH the pre-auth/authenticated gate below and, on the authenticated
    // branch, the principal actually bound to the socket. Resolving early
    // is what lets a device_id-bound principal (mTLS client-cert CN
    // matching `[[authz.principals]].device_ids`) skip pre-auth even with
    // no pairing token at all — see `connection_is_authenticated`.
    let resolved = resolve_principal(
        &state.provider_registry,
        token.as_deref(),
        device_id.as_deref(),
    )
    .await;

    // Pairing-over-ACP (fork-local): when pairing is required and the
    // presented token is absent or not (yet) paired, do NOT 401 — upgrade
    // the connection in pre-auth mode instead, UNLESS the credential
    // already resolved to a DISTINCT authenticated principal (condition 3
    // below). A pre-auth connection may call only `zeroclaw/pair`; on a
    // successful pair it resolves its principal and proceeds on the SAME
    // socket (see `handle_socket`). This is the only branch that departs
    // from today's behavior: a deployment with `require_pairing = false`,
    // a connection presenting an already-paired token, or a connection
    // whose mTLS device_id already resolved to a distinct principal, takes
    // the authenticated path exactly as before.
    let authenticated = connection_is_authenticated(
        state.pairing.require_pairing(),
        token
            .as_deref()
            .is_some_and(|t| state.pairing.is_authenticated(t)),
        resolved.as_ref(),
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
        // lockout the same way the REST `/pair` and channel front doors key
        // it: by peer identity.
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
    // deployments) is unchanged. Unchanged from before pairing-over-ACP: a
    // presented-but-Denied bearer is still rejected with a hard 401 here.
    let authz_enforced = state.config.read().authz.is_enforced();
    let principal = match resolved {
        Some(principal) => principal,
        None if !authz_enforced => Principal::shared_operator(),
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
            ConnAuth::Authenticated(principal),
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
///    (`resolved.is_authenticated()`) — e.g. an mTLS client-cert `device_id`
///    matching a `[[authz.principals]].device_ids` entry under enforced
///    authz.
///
/// Condition 3 can never be satisfied by [`Principal::shared_operator`]
/// (`is_authenticated() == false`), so a bare CA-issued client certificate
/// can NEVER bypass `require_pairing` when authz is NOT enforced — only a
/// config-backed *distinct* principal (i.e. authz IS enforced and the
/// device_id/token actually maps to a configured principal) skips pairing.
fn connection_is_authenticated(
    require_pairing: bool,
    token_is_paired: bool,
    resolved: Option<&Principal>,
) -> bool {
    !require_pairing || token_is_paired || resolved.is_some_and(Principal::is_authenticated)
}

/// The connection's auth state at the moment its socket is handed to
/// `handle_socket`. `PreAuth` is resolved into `Authenticated` in-place, on
/// the same connection, by a successful `zeroclaw/pair` — see
/// `run_pre_auth`.
enum ConnAuth {
    Authenticated(Principal),
    PreAuth,
}

/// Build the ACP connection auth registry from the fork-local `[[authz]]` map.
/// Registers the [`PairingAuthProvider`], which maps a paired bearer to a
/// principal when authz is enforced and to the shared-operator sentinel
/// otherwise. Built once at daemon start and shared via [`AppState`].
#[must_use]
pub fn build_provider_registry(
    authz: AuthzConfig,
    bindings: Arc<zeroclaw_config::authz::TokenBindingStore>,
) -> Arc<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(PairingAuthProvider::new(authz, bindings)));
    Arc::new(registry)
}

/// Resolve a presented bearer and/or mTLS `device_id` to a [`Principal`] via
/// the registry. `None` means the credential was denied (or absent); the
/// caller decides whether that is a hard reject (authz enforced) or the
/// shared-operator fallback (legacy).
///
/// `device_id` (the client cert's subject CN, when the connection is mTLS) is
/// threaded alongside `token` so `[[authz.principals]].device_ids`-based
/// principals can bind even when no bearer was presented.
pub async fn resolve_principal(
    registry: &ProviderRegistry,
    token: Option<&str>,
    device_id: Option<&str>,
) -> Option<Principal> {
    let token = token.filter(|t| !t.is_empty()).map(str::to_string);
    let credential = match (device_id, token) {
        (Some(device_id), token) => Credential::Mtls {
            device_id: device_id.to_string(),
            token,
        },
        (None, Some(token)) => Credential::Bearer(token),
        (None, None) => Credential::None,
    };
    match registry.resolve(&credential).await {
        AuthOutcome::Authenticated(principal) | AuthOutcome::Trusted(principal) => Some(principal),
        AuthOutcome::Denied { .. } => None,
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

    let principal = match auth {
        ConnAuth::Authenticated(principal) => principal,
        ConnAuth::PreAuth => {
            match run_pre_auth(&mut sender, &mut receiver, &state, &client_id).await {
                Some(principal) => principal,
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
                acp_config,
                output_tx,
                store,
            )
            .with_canvas_store(canvas_store)
            .with_sop_engine(state.sop_engine.clone(), state.sop_audit.clone())
            .with_connection_default_agent(default_agent)
            .with_principal(principal),
        )
    } else {
        Arc::new(
            AcpServer::new_with_live_config_and_writer(
                Arc::clone(&state.config),
                acp_config,
                output_tx,
            )
            .with_canvas_store(canvas_store)
            .with_sop_engine(state.sop_engine.clone(), state.sop_audit.clone())
            .with_connection_default_agent(default_agent)
            .with_principal(principal),
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
/// JSON-RPC frames directly off the socket (no `AcpServer`, no session — none
/// is constructed until a principal is resolved). The only accepted method is
/// `zeroclaw/pair`; every other method gets an `AuthRequired` error and the
/// loop keeps waiting. Returns the resolved [`Principal`] on a successful
/// pair (the caller then proceeds on the SAME socket, reusing `sender` and
/// `receiver`), or `None` if the socket closes, errors, a write fails, or the
/// handshake timeout elapses first.
async fn run_pre_auth(
    sender: &mut SplitSink<WebSocket, Message>,
    receiver: &mut SplitStream<WebSocket>,
    state: &AppState,
    client_id: &str,
) -> Option<Principal> {
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

        if method != "zeroclaw/pair" {
            if !send_pre_auth_error(
                sender,
                id,
                error_codes::AUTH_REQUIRED,
                "pair first via zeroclaw/pair",
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
                // A principal-tagged code (`get-paircode --new --principal
                // <id>`) stashes a binding that must be drained into the live
                // runtime store before it resolves — mirrors the REST
                // `/pair` handler in `lib.rs` so a principal-tagged code
                // works identically whether redeemed over `/pair` or over
                // `zeroclaw/pair`.
                if let Some(binding) = state.pairing.take_pending_binding() {
                    let _ = state
                        .token_bindings
                        .set(binding.token_hash, binding.principal_id);
                }

                let authz_enforced = state.config.read().authz.is_enforced();
                let principal =
                    match resolve_principal(&state.provider_registry, Some(&token), None).await {
                        Some(principal) => principal,
                        None if !authz_enforced => Principal::shared_operator(),
                        None => {
                            // Paired, but the issued token is not entitled
                            // under enforced authz (e.g. a stale/removed
                            // principal binding). Stay pre-auth rather than
                            // proceeding with no principal.
                            if !send_pre_auth_error(
                            sender,
                            id,
                            error_codes::AUTH_REQUIRED,
                            "pairing succeeded but no principal is entitled for the issued token",
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
                return Some(principal);
            }
            Ok(None) => {
                if !send_pre_auth_error(
                    sender,
                    id,
                    error_codes::AUTH_REQUIRED,
                    "invalid pairing code",
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
    send_pre_auth_response(
        sender,
        id,
        None,
        Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
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
        let reload_controls = zeroclaw_runtime::daemon::GatewayReloadControls {
            shutdown_tx: shutdown_tx.clone(),
            reload_tx,
        };

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

    #[tokio::test]
    async fn resolve_denies_unmapped_when_enforced() {
        use zeroclaw_config::authz::{AuthzConfig, PrincipalRecord};
        use zeroclaw_config::pairing::PairingGuard;

        let authz = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec!["crm-bot".into()],
                device_ids: vec![],
                token_hashes: vec![PairingGuard::token_hash("good")],
                profiles: vec![],
            }],
            profiles: vec![],
        };
        let registry = super::build_provider_registry(
            authz,
            std::sync::Arc::new(zeroclaw_config::authz::TokenBindingStore::new_ephemeral()),
        );

        assert!(
            super::resolve_principal(&registry, Some("bad"), None)
                .await
                .is_none(),
            "an unmapped bearer must be denied once authz is enforced"
        );

        let principal = super::resolve_principal(&registry, Some("good"), None)
            .await
            .expect("the mapped bearer must resolve to a principal");
        assert!(principal.may_bind("crm-bot"));
        assert!(!principal.may_bind("hr-bot"));
    }

    #[tokio::test]
    async fn resolve_principal_binds_by_device_id_with_no_token() {
        use zeroclaw_config::authz::{AuthzConfig, PrincipalRecord};

        let authz = AuthzConfig {
            principals: vec![PrincipalRecord {
                id: "alice".into(),
                allowed_agents: vec!["crm-bot".into()],
                device_ids: vec!["dev-abc".into()],
                token_hashes: vec![],
                profiles: vec![],
            }],
            profiles: vec![],
        };
        let registry = super::build_provider_registry(
            authz,
            std::sync::Arc::new(zeroclaw_config::authz::TokenBindingStore::new_ephemeral()),
        );

        assert!(
            super::resolve_principal(&registry, None, Some("dev-nope"))
                .await
                .is_none(),
            "a non-matching device_id must be denied once authz is enforced"
        );

        let principal = super::resolve_principal(&registry, None, Some("dev-abc"))
            .await
            .expect("the matching device_id must resolve to a principal, with no token needed");
        assert!(principal.may_bind("crm-bot"));
    }

    // --- connection_is_authenticated (the pre-auth/authenticated gate) ---

    #[test]
    fn legacy_no_pairing_required_is_always_authenticated() {
        // Condition 1: require_pairing = false short-circuits regardless of
        // token/resolved state — unchanged legacy behavior.
        assert!(super::connection_is_authenticated(false, false, None));
    }

    #[test]
    fn paired_token_is_authenticated_even_with_no_resolved_principal() {
        // Condition 2: the existing PairingGuard token-paired path, untouched
        // by the device_id feature.
        assert!(super::connection_is_authenticated(true, true, None));
    }

    #[test]
    fn distinct_resolved_principal_authenticates_under_required_pairing() {
        // Condition 3 (NEW): under require_pairing = true, with no paired
        // token, a distinct principal already resolved (e.g. via a matching
        // mTLS device_id under enforced authz) is sufficient to skip
        // pre-auth — this is the enterprise-default fix.
        use zeroclaw_api::principal::{AuthMethod, Principal, PrincipalId};

        let principal = Principal::new(
            PrincipalId::from("alice".to_string()),
            "alice".to_string(),
            AuthMethod::Native,
        );
        assert!(principal.is_authenticated());
        assert!(super::connection_is_authenticated(
            true,
            false,
            Some(&principal)
        ));
    }

    #[test]
    fn shared_operator_never_bypasses_required_pairing() {
        // The safety property the review validated: resolve_principal falls
        // back to shared_operator() when authz is NOT enforced (e.g. a bare
        // mTLS cert with no matching principal, or no cert at all). That
        // sentinel is never "distinct" (`is_authenticated() == false`), so it
        // must NOT satisfy condition 3 — a bare CA cert can never bypass
        // require_pairing this way.
        use zeroclaw_api::principal::Principal;

        let shared = Principal::shared_operator();
        assert!(!shared.is_authenticated());
        assert!(!super::connection_is_authenticated(
            true,
            false,
            Some(&shared)
        ));
    }

    #[test]
    fn no_resolved_principal_does_not_authenticate_under_required_pairing() {
        // A device_id (or token) matching nothing at all — `resolved` is
        // `None` — must still fall through to pre-auth (or, on the
        // authenticated branch reached via another condition, to the
        // existing 401/shared-operator handling). It never satisfies
        // condition 3.
        assert!(!super::connection_is_authenticated(true, false, None));
    }

    /// End-to-end composition of resolve_principal + connection_is_authenticated
    /// for the three scenarios the security review asked to be pinned down:
    /// enforced authz + matching device_id skips pre-auth and binds the
    /// principal; enforced authz + non-matching device_id stays gated;
    /// UNenforced authz + a device_id never bypasses required pairing.
    #[tokio::test]
    async fn device_id_gate_composition_matches_reviewed_rule() {
        use zeroclaw_config::authz::{AuthzConfig, PrincipalRecord};

        let enforced_registry = super::build_provider_registry(
            AuthzConfig {
                principals: vec![PrincipalRecord {
                    id: "alice".into(),
                    allowed_agents: vec!["crm-bot".into()],
                    device_ids: vec!["dev-abc".into()],
                    token_hashes: vec![],
                    profiles: vec![],
                }],
                profiles: vec![],
            },
            std::sync::Arc::new(zeroclaw_config::authz::TokenBindingStore::new_ephemeral()),
        );

        // Enforced authz + matching device_id, no token, require_pairing=true
        // -> resolves to a distinct principal -> gate is authenticated.
        let matched = super::resolve_principal(&enforced_registry, None, Some("dev-abc")).await;
        assert!(super::connection_is_authenticated(
            true,
            false,
            matched.as_ref()
        ));
        assert!(matched.unwrap().may_bind("crm-bot"));

        // Enforced authz + non-matching device_id -> Denied (None) -> gate
        // stays pre-auth.
        let unmatched = super::resolve_principal(&enforced_registry, None, Some("dev-nope")).await;
        assert!(unmatched.is_none());
        assert!(!super::connection_is_authenticated(
            true,
            false,
            unmatched.as_ref()
        ));

        // UNenforced authz + a device_id (no matching config at all) ->
        // shared_operator, not distinct -> gate stays pre-auth even though
        // resolve_principal returned Some.
        let unenforced_registry = super::build_provider_registry(
            AuthzConfig::default(),
            std::sync::Arc::new(zeroclaw_config::authz::TokenBindingStore::new_ephemeral()),
        );
        let bare_cert =
            super::resolve_principal(&unenforced_registry, None, Some("any-cert-cn")).await;
        assert!(bare_cert.as_ref().is_some_and(|p| !p.is_authenticated()));
        assert!(!super::connection_is_authenticated(
            true,
            false,
            bare_cert.as_ref()
        ));
    }

    /// `front_door_config` with pairing turned on and no pre-paired tokens,
    /// so `run_gateway` mints a fresh one-time code at boot (mirrors the
    /// `PairingGuard::new` "no tokens yet" branch).
    fn pairing_required_front_door_config(
        install_root: &std::path::Path,
    ) -> zeroclaw_config::schema::Config {
        let mut cfg = front_door_config(install_root);
        cfg.gateway.require_pairing = true;
        cfg
    }

    /// Fetch the live one-time pairing code the same way an operator or the
    /// CLI does — `GET /pair/code` — without pulling in an HTTP client
    /// dependency: a bare TCP request with `Connection: close` so the server
    /// closes the socket once the response is fully written, then the JSON
    /// body is the tail end of the response past the header/body blank line.
    async fn fetch_pairing_code(addr: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connect for GET /pair/code");
        let request =
            format!("GET /pair/code HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write GET /pair/code request");
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .await
            .expect("read GET /pair/code response");
        let response = String::from_utf8_lossy(&raw).into_owned();
        let head_end = response
            .find("\r\n\r\n")
            .map(|i| i + 4)
            .expect("HTTP response must have a header/body separator");
        let tail = &response[head_end..];
        // Locate the JSON object by its braces rather than assuming the body
        // is unwrapped: hyper may send it chunked (a hex length line before
        // the object, a "0\r\n\r\n" terminator after), and slicing on braces
        // is robust to either framing.
        let start = tail.find('{').expect("response body must contain JSON");
        let end = tail.rfind('}').expect("response body must contain JSON") + 1;
        let value: serde_json::Value =
            serde_json::from_str(&tail[start..end]).expect("/pair/code body must be JSON");
        value["pairing_code"]
            .as_str()
            .expect("pairing_code must be exposed before the first pairing")
            .to_string()
    }

    /// Pairing-over-ACP front-door proof: an unauthenticated `/acp`
    /// connection may call ONLY `zeroclaw/pair`; a successful pair resolves
    /// its principal and lets the SAME connection proceed — no reconnect.
    /// Drives the real route exactly like the sibling front-door test above.
    #[tokio::test]
    async fn acp_unauth_connection_only_allows_pairing() {
        let tmp = tempfile::tempdir().unwrap();
        let install_root = tmp.path();
        let cfg = pairing_required_front_door_config(install_root);
        std::fs::create_dir_all(&cfg.data_dir).unwrap();

        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        let (reload_tx, _reload_rx) = tokio::sync::watch::channel(false);
        let reload_controls = zeroclaw_runtime::daemon::GatewayReloadControls {
            shutdown_tx: shutdown_tx.clone(),
            reload_tx,
        };

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

        // The one-time code is exposed via GET /pair/code before the first
        // successful pairing — capture it the same way an operator/CLI would.
        let code = fetch_pairing_code(&addr).await;

        let url = format!("ws://{addr}/acp");
        let (mut ws, _resp) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WebSocket upgrade on /acp must succeed even when unauthenticated");

        // (a) An unauth non-pair method is rejected with an AuthRequired
        // error, never reaching a session.
        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc":"2.0","id":1,"method":"session/new",
                "params":{"agentAlias":"test-agent"}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        let rejected = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let msg = ws
                    .next()
                    .await
                    .expect("socket closed before session/new rejection")
                    .unwrap();
                let Message::Text(text) = msg else { continue };
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if value.get("id").and_then(serde_json::Value::as_i64) == Some(1) {
                    return value;
                }
            }
        })
        .await
        .expect("session/new rejection should arrive before timeout");
        assert!(
            rejected.get("error").is_some(),
            "unauth session/new must be rejected with a JSON-RPC error: {rejected}"
        );
        assert!(
            rejected.get("result").is_none(),
            "unauth session/new must not create a session: {rejected}"
        );

        // (b) zeroclaw/pair with the live code returns a token on the SAME
        // connection.
        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc":"2.0","id":2,"method":"zeroclaw/pair",
                "params":{"code":code}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        let token = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let msg = ws
                    .next()
                    .await
                    .expect("socket closed before pair response")
                    .unwrap();
                let Message::Text(text) = msg else { continue };
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if value.get("id").and_then(serde_json::Value::as_i64) == Some(2) {
                    if let Some(err) = value.get("error") {
                        panic!("zeroclaw/pair with the live code returned an error: {err}");
                    }
                    return value["result"]["token"].as_str().map(String::from);
                }
            }
        })
        .await
        .expect("pair response should arrive before timeout")
        .expect("zeroclaw/pair with the live code must return a token");
        assert!(!token.is_empty(), "the issued token must be non-empty");

        // (c) The SAME connection can now call session/new — no reconnect.
        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc":"2.0","id":3,"method":"session/new",
                "params":{"agentAlias":"test-agent"}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        let session_response = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let msg = ws
                    .next()
                    .await
                    .expect("socket closed before session/new response")
                    .unwrap();
                let Message::Text(text) = msg else { continue };
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if value.get("id").and_then(serde_json::Value::as_i64) == Some(3) {
                    return value;
                }
            }
        })
        .await
        .expect("session/new response should arrive before timeout");

        shutdown_tx.send(true).ok();
        server.abort();

        assert!(
            session_response.get("error").is_none(),
            "post-pair session/new on the SAME connection must succeed: {session_response}"
        );
        assert!(
            session_response["result"]["workspaceDir"].is_string(),
            "post-pair session/new must return a session: {session_response}"
        );
    }
}
