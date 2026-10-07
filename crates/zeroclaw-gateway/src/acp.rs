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
use base64::Engine as _;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc;
use zeroclaw_api::grants::ResolvedGrants;
use zeroclaw_api::jsonrpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcResponse, error_codes};
use zeroclaw_api::principal::{AuthOutcome, AuthenticatedIdentity, Principal};
use zeroclaw_channels::orchestrator::acp_server::{AcpServer, AcpServerConfig};
use zeroclaw_config::schema::Config;
use zeroclaw_infra::acp_session_store::AcpSessionStore;
use zeroclaw_runtime::security::auth_provider::{
    AuthProvider, Credential, OidcAuthProvider, ProviderRegistry,
};
use zeroclaw_runtime::security::pairing_auth_provider::PairingAuthProvider;
use zeroclaw_runtime::security::principal_resolver::PrincipalResolver;

/// The ACP WebSocket subprotocols this endpoint speaks, in preference
/// order: the fork's own name first, upstream's second. Both carry the
/// identical frame contract; a client may offer either (or both, alongside
/// its `bearer.<token>` entry), and the gateway echoes back the one it
/// picked so a browser/WebView2 handshake — which aborts when the server
/// answers with no `Sec-WebSocket-Protocol` — completes.
const ACP_WS_PROTOCOLS: [&str; 2] = ["volt.acp.v1", "zeroclaw.acp.v1"];

/// Pick the subprotocol to echo for a client's `Sec-WebSocket-Protocol`
/// offer: `volt.acp.v1` when offered (even if `zeroclaw.acp.v1` is offered
/// too, regardless of the client's order), else `zeroclaw.acp.v1` when
/// offered, else `None` (nothing is echoed; a client that offered only a
/// `bearer.<token>` entry, or nothing, upgrades without a subprotocol as
/// before).
#[must_use]
fn select_acp_subprotocol(offered: Option<&str>) -> Option<&'static str> {
    let offered = offered?;
    ACP_WS_PROTOCOLS
        .into_iter()
        .find(|known| offered.split(',').any(|p| p.trim() == *known))
}

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
    // binding-store writes happen on this path); its outcome, the pairing
    // state of the token and the enforcement mode together decide the
    // upgrade (see `upgrade_decision`).
    let resolution = resolve_principal(&state, token.as_deref()).await;
    let token_is_paired = token
        .as_deref()
        .is_some_and(|t| state.pairing.is_authenticated(t));
    let authz_enforced = authz_enforced(&state.config.read());
    let decision = upgrade_decision(
        resolution,
        state.pairing.require_pairing(),
        token_is_paired,
        authz_enforced,
    );

    // Exactly one known subprotocol is handed to axum, so the echo is the
    // one `select_acp_subprotocol` chose rather than whichever the client
    // happened to list first.
    let ws = match select_acp_subprotocol(
        headers
            .get(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
            .and_then(|v| v.to_str().ok()),
    ) {
        Some(protocol) => ws.protocols([protocol]),
        None => ws,
    };

    let client_id = peer_addr.to_string();
    match decision {
        // Pre-auth: no principal to resolve yet, no 401 — the connection
        // itself is allowed; only its method set is restricted, enforced in
        // `handle_socket`. `client_id` buckets `PairingGuard`'s brute-force
        // lockout the same way the REST `/pair` front door keys it: by peer
        // identity.
        UpgradeDecision::PreAuth => ws
            .on_upgrade(move |socket| {
                handle_socket(socket, state, params.agent, client_id, ConnAuth::PreAuth)
            })
            .into_response(),
        UpgradeDecision::Unauthorized => (
            axum::http::StatusCode::UNAUTHORIZED,
            "Unauthorized - no principal is entitled for the presented credential",
        )
            .into_response(),
        UpgradeDecision::Authenticated(principal_and_grants) => ws
            .on_upgrade(move |socket| {
                handle_socket(
                    socket,
                    state,
                    params.agent,
                    client_id,
                    ConnAuth::Authenticated(principal_and_grants),
                )
            })
            .into_response(),
    }
}

/// What the `/acp` upgrade does with a [`Resolution`] — the pure decision
/// behind [`handle_ws_acp`], so the frozen contract is unit-testable
/// without a socket.
enum UpgradeDecision {
    /// Upgrade with this subject bound to the socket.
    Authenticated(Box<(Principal, ResolvedGrants)>),
    /// Upgrade in pre-auth mode (only `zeroclaw/pair`/`volt/pair`).
    PreAuth,
    /// HTTP 401 at the upgrade.
    Unauthorized,
}

/// The `/acp` upgrade decision (frozen client contract):
///
/// * [`Resolution::VerifiedNotEntitled`] ⇒ `401`, settled BEFORE the
///   pre-auth gate: the credential is real (an SSO user whose group maps to
///   nothing here, a roster principal with no agent), so pairing again
///   cannot help and the shared operator is never a fallback for a verified
///   identity.
/// * [`Resolution::Resolved`] ⇒ authenticated (a distinct principal also
///   satisfies the pairing gate on its own — `connection_is_authenticated`
///   condition 3).
/// * [`Resolution::Denied`] ⇒ pre-auth when pairing is required and the
///   token is not paired; otherwise the legacy shared-operator fallback
///   when authz is NOT enforced, else `401`.
fn upgrade_decision(
    resolution: Resolution,
    require_pairing: bool,
    token_is_paired: bool,
    authz_enforced: bool,
) -> UpgradeDecision {
    let resolved = match resolution {
        Resolution::VerifiedNotEntitled => return UpgradeDecision::Unauthorized,
        Resolution::Resolved(principal_and_grants) => Some(principal_and_grants),
        Resolution::Denied => None,
    };
    // Pairing-over-ACP (fork-local): when pairing is required and the
    // presented token is absent or not (yet) paired, do NOT 401 — upgrade
    // the connection in pre-auth mode instead, UNLESS the credential
    // already resolved to a DISTINCT authenticated principal. A deployment
    // with `require_pairing = false`, or a connection presenting an
    // already-paired token, takes the authenticated path exactly as before.
    if !connection_is_authenticated(
        require_pairing,
        token_is_paired,
        resolved.as_deref().map(|(principal, _)| principal),
    ) {
        return UpgradeDecision::PreAuth;
    }
    // Fail-closed once authz is enforced; otherwise fall back to the shared
    // operator so the legacy single-operator path (including no-pairing
    // deployments) is unchanged. A presented-but-Denied bearer is still
    // rejected with a hard 401 here.
    match resolved {
        Some(principal_and_grants) => UpgradeDecision::Authenticated(principal_and_grants),
        None if !authz_enforced => UpgradeDecision::Authenticated(Box::new((
            Principal::shared_operator(),
            ResolvedGrants::all(),
        ))),
        None => UpgradeDecision::Unauthorized,
    }
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
    /// Boxed: the authenticated payload is ~240 bytes against a unit
    /// `PreAuth`, and the value is built once per connection.
    Authenticated(Box<(Principal, ResolvedGrants)>),
    PreAuth,
}

/// Whether `/acp` is in fail-closed (distinct-principal) mode: any
/// `[[authz.principals]]` roster entry OR any `[oidc.<alias>]` trust
/// relationship turns enforcement on. An OIDC-only deployment (no pairing
/// principals at all) must never fall back to the shared operator either.
#[must_use]
pub(crate) fn authz_enforced(config: &Config) -> bool {
    config.authz.is_enforced() || !config.oidc.is_empty()
}

/// The provider class a presented bearer belongs to, decided from the
/// credential's FORMAT (upstream's registry selects providers by name and
/// has no fallback chain; `/acp` clients name no provider — `bearer.<token>`
/// is all the frozen client contract carries — so the gateway routes by
/// shape). A string that parses as a JWT (three non-empty dot-separated
/// segments whose header is base64url JSON carrying `alg`) is an OIDC
/// access token and NEVER a pairing token; everything else is a pairing
/// token (`zc_…`).
#[derive(Debug, PartialEq, Eq)]
enum BearerShape {
    Pairing,
    /// A JWT; `issuer` is the UNVERIFIED `iss` claim, used only to pick
    /// which `oidc.<alias>` provider verifies it (the provider re-checks
    /// `iss` against its configured issuer after signature validation).
    Jwt {
        issuer: Option<String>,
    },
}

fn classify_bearer(token: &str) -> BearerShape {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return BearerShape::Pairing;
    };
    if header.is_empty() || payload.is_empty() || signature.is_empty() {
        return BearerShape::Pairing;
    }
    let decode = |segment: &str| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(segment)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    };
    let Some(header) = decode(header) else {
        return BearerShape::Pairing;
    };
    if !header.get("alg").is_some_and(Value::is_string) {
        return BearerShape::Pairing;
    }
    let issuer = decode(payload)
        .and_then(|claims| claims.get("iss").and_then(Value::as_str).map(str::to_owned));
    BearerShape::Jwt { issuer }
}

/// Pick the registry name that must verify `credential`: `"pairing"` for
/// pairing tokens (and for an absent credential, which the pairing provider
/// denies itself), `oidc.<alias>` for a JWT whose `iss` matches that
/// alias's configured issuer. `None` = no provider may see it (a JWT for an
/// issuer this daemon does not trust) — the caller denies.
fn select_provider(credential: &Credential, config: &Config) -> Option<String> {
    let Credential::Bearer(token) = credential else {
        return Some("pairing".to_owned());
    };
    match classify_bearer(token) {
        BearerShape::Pairing => Some("pairing".to_owned()),
        BearerShape::Jwt { issuer } => {
            let issuer = issuer?;
            // Deterministic when two aliases name one issuer (config
            // validation rejects that, but the load path tolerates an
            // invalid config so an operator can boot to repair it).
            let mut aliases: Vec<&String> = config
                .oidc
                .iter()
                .filter(|(_, oidc)| oidc.issuer == issuer)
                .map(|(alias, _)| alias)
                .collect();
            aliases.sort();
            aliases.first().map(|alias| format!("oidc.{alias}"))
        }
    }
}

/// Per-alias cache of the `oidc.<alias>` providers the ACP registry
/// reuses across connections, keyed by the alias and pinned to the exact
/// config it was built from. An [`OidcAuthProvider`] owns its discovery
/// and JWKS caches (bounded refresh cooldowns, key-rotation handling), so
/// rebuilding one per connection would re-fetch the issuer's documents on
/// every `/acp` connect; a config edit changes the fingerprint and
/// transparently rebuilds just that alias.
static ACP_OIDC_PROVIDERS: OnceLock<OidcProviderCache> = OnceLock::new();

/// alias → (config fingerprint, provider built from that config).
type OidcProviderCache = Mutex<HashMap<String, (Value, Arc<OidcAuthProvider>)>>;

/// The `oidc.<alias>` providers for the live config, rebuilt only for
/// aliases whose config changed. An alias whose provider cannot be
/// constructed is skipped (and logged): a JWT for that issuer then finds no
/// provider and is denied — fail closed, never fall through to pairing.
fn oidc_providers(config: &Config) -> Vec<Arc<OidcAuthProvider>> {
    let cache = ACP_OIDC_PROVIDERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock();
    cache.retain(|alias, _| config.oidc.contains_key(alias));
    let mut aliases: Vec<&String> = config.oidc.keys().collect();
    aliases.sort();
    let mut providers = Vec::with_capacity(aliases.len());
    for alias in aliases {
        let oidc = &config.oidc[alias];
        let fingerprint = match serde_json::to_value(oidc) {
            Ok(value) => value,
            Err(e) => {
                ::zeroclaw_log::record!(
                    WARN,
                    ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                        .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                        .with_attrs(::serde_json::json!({"alias": alias, "error": e.to_string()})),
                    "ACP OIDC: cannot fingerprint [oidc.<alias>] config; provider skipped (fail closed)"
                );
                continue;
            }
        };
        if let Some((cached_fingerprint, provider)) = cache.get(alias)
            && *cached_fingerprint == fingerprint
        {
            providers.push(Arc::clone(provider));
            continue;
        }
        match OidcAuthProvider::new(alias.clone(), oidc.clone()) {
            Ok(provider) => {
                let provider = Arc::new(provider);
                cache.insert(alias.clone(), (fingerprint, Arc::clone(&provider)));
                providers.push(provider);
            }
            Err(e) => {
                cache.remove(alias);
                ::zeroclaw_log::record!(
                    WARN,
                    ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                        .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                        .with_attrs(::serde_json::json!({"alias": alias, "error": e.to_string()})),
                    "ACP OIDC: failed to build the oidc.<alias> provider; its tokens are denied (fail closed)"
                );
            }
        }
    }
    providers
}

/// Build the ACP-dedicated connection auth registry: the
/// [`PairingAuthProvider`] under the name `"pairing"` plus one
/// `oidc.<alias>` provider per `[oidc.<alias>]` entry (the same
/// [`OidcAuthProvider`] upstream registers on its own RPC transports) —
/// never upstream's `"native"` provider, which maps every paired token to
/// the shared operator. Kept as its own registry (rather than joining the
/// daemon's main one) to minimize blast radius: ACP's bearer-to-principal
/// resolution stays self-contained. Built per connection from the live
/// config snapshot, so a config edit (new/removed principal, rebound
/// profile, changed issuer) applies with no reload; the OIDC providers
/// themselves are cached (see [`oidc_providers`]).
#[must_use]
fn build_provider_registry(
    config: &Config,
    bindings: Arc<zeroclaw_config::authz::TokenBindingStore>,
) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    if let Err(e) = registry.register(Arc::new(PairingAuthProvider::new(
        config.authz.clone(),
        bindings,
    ))) {
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
    for provider in oidc_providers(config) {
        let name = provider.name().to_owned();
        if let Err(e) = registry.register(provider) {
            ::zeroclaw_log::record!(
                ERROR,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"provider": name, "error": e.to_string()})),
                "failed to register an ACP OIDC auth provider; its tokens are denied"
            );
        }
    }
    registry
}

/// Whether a pairing-shaped bearer may be handed to the pairing provider at
/// all: when pairing is required it must currently be a paired token
/// (`PairingGuard::is_authenticated`, which is `true` unconditionally when
/// pairing is NOT required — that deployment trusts its transport and the
/// provider's own pin/binding lookup is the only gate). Pure so the
/// revocation contract is unit-testable without a socket.
#[must_use]
fn pairing_bearer_is_live(require_pairing: bool, token_is_paired: bool) -> bool {
    !require_pairing || token_is_paired
}

/// Drop the `TokenBindingStore` entry for a token that was just revoked —
/// rotate-device, `DELETE /api/pairing/devices/..`, or a pairing rolled
/// back after a failed registry/persist step. Companion to every
/// `PairingGuard::revoke_token_hash` call in the gateway. The revocation
/// itself is the paired-set removal (`resolve_principal` refuses an unpaired
/// bearer regardless of bindings); this keeps the on-disk
/// `token_hash -> principal_id` map from accumulating rows that no paired
/// token can ever match again. A persist failure is logged, never fatal: the
/// in-memory removal already happened.
pub(crate) fn unbind_token_hash(state: &AppState, token_hash: &str) {
    if let Err(e) = state.token_bindings.remove(token_hash) {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": e.to_string()})),
            "token revoked but persisting the token->principal binding removal failed; the binding is gone in-process"
        );
    }
}

/// Companion to `PairingGuard::revoke_all_tokens` (rotate-all): no paired
/// token is left, so no binding may remain either. Same logging posture as
/// [`unbind_token_hash`].
pub(crate) fn unbind_all_tokens(state: &AppState) {
    if let Err(e) = state.token_bindings.clear() {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": e.to_string()})),
            "all tokens revoked but persisting the token->principal binding clear failed; the bindings are gone in-process"
        );
    }
}

/// Drain the binding a just-redeemed, principal-tagged pairing code left
/// behind (`PairingGuard::take_pending_binding`) into the live
/// `TokenBindingStore`, so the new bearer resolves to that
/// `[[authz.principals]]` record from its very next connect. Shared by every
/// pairing redemption point (in-band `zeroclaw/pair`, REST `/pair`, enhanced
/// `/api/pair`). Returns the binding (if any) so REST callers can echo it.
/// A persist failure is logged, not fatal: the binding is live in-process
/// and the device is paired; the next redemption/restart retries.
pub(crate) fn bind_pending_principal(
    state: &AppState,
) -> Option<zeroclaw_config::pairing::PrincipalBinding> {
    let binding = state.pairing.take_pending_binding()?;
    match state
        .token_bindings
        .set(binding.token_hash.clone(), binding.principal_id.clone())
    {
        Ok(()) => ::zeroclaw_log::record!(
            INFO,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_attrs(::serde_json::json!({ "principal_id": binding.principal_id })),
            "device paired with a principal-tagged code; token bound to principal in the runtime binding store"
        ),
        Err(e) => ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({
                    "principal_id": binding.principal_id,
                    "error": e.to_string(),
                })),
            "device paired with a principal-tagged code but persisting the token->principal binding failed; the binding is active in-process"
        ),
    }
    Some(binding)
}

/// Outcome of resolving a presented bearer (see [`resolve_principal`]).
/// Three-way on purpose: a caller must be able to tell a credential that
/// was never accepted (pre-auth / legacy fallback territory) from one that
/// WAS verified but has nothing here (a hard `401`, never pre-auth, never
/// the shared operator).
pub(crate) enum Resolution {
    /// Verified and entitled: a distinct principal with at least one agent
    /// (or admin). Boxed like `ConnAuth::Authenticated` (~240 bytes against
    /// two unit variants).
    Resolved(Box<(Principal, ResolvedGrants)>),
    /// The provider verified the credential, but the resolver denied it
    /// (not in the roster, misconfigured policy, ...) OR its resolved
    /// grants cover no agent at all (`!admin && allowed_agents.is_empty()`).
    VerifiedNotEntitled,
    /// Not verified: absent credential, a JWT from an untrusted issuer, a
    /// pairing token that is not (or no longer) paired, a provider denial,
    /// or a policy that failed to compile.
    Denied,
}

/// Resolve a presented bearer to a [`Principal`] + its [`ResolvedGrants`]
/// via the ACP-dedicated registry (the provider [`select_provider`] picks
/// by credential shape: `"pairing"` or `oidc.<alias>`) and the shared
/// [`PrincipalResolver`]. See [`Resolution`] for the three outcomes; the
/// caller decides what a denial means (hard reject under enforced authz,
/// pre-auth, or the legacy shared-operator fallback).
pub(crate) async fn resolve_principal(state: &AppState, token: Option<&str>) -> Resolution {
    let token = token.filter(|t| !t.is_empty()).map(str::to_string);
    let credential = match token {
        Some(token) => Credential::Bearer(token),
        None => Credential::None,
    };
    let config = state.config.read().clone();
    let Some(provider) = select_provider(&credential, &config) else {
        // A JWT for an issuer this daemon does not trust (or with no `iss`
        // at all). It is never handed to the pairing provider.
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure),
            "ACP: bearer is a JWT from an issuer with no [oidc.<alias>] trust entry; denying"
        );
        return Resolution::Denied;
    };
    // Pairing liveness, fail-closed: a bearer routed to the pairing provider
    // must ALSO still be in the paired set whenever pairing is required. A
    // `[[authz.principals]].token_hashes` pin or a `TokenBindingStore`
    // binding names a token by hash, but neither IS a pairing — once the
    // operator revokes the token (`get-paircode --rotate`/`--rotate-device`,
    // `DELETE /api/pairing/devices/..`, a config edit removing it from
    // `gateway.paired_tokens`) it must stop resolving even though the
    // pin/binding may still name it.
    if provider == "pairing"
        && let Credential::Bearer(token) = &credential
        && !pairing_bearer_is_live(
            state.pairing.require_pairing(),
            state.pairing.is_authenticated(token),
        )
    {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure),
            "ACP: pairing-shaped bearer is not (or no longer) a paired token; denying"
        );
        return Resolution::Denied;
    }
    let registry = build_provider_registry(&config, Arc::clone(&state.token_bindings));
    let outcome = registry.resolve_named(&provider, &credential).await;
    let identity = match outcome {
        AuthOutcome::Verified(identity) => identity,
        AuthOutcome::Denied { .. } => return Resolution::Denied,
    };
    admit(state, &config, &identity)
}

/// Resolve a VERIFIED identity to its principal and grants against the
/// live policy, refusing one entitled to no agent. An admitted OIDC
/// identity is recorded in the gateway's external-subjects registry, the
/// same way the REST gate records a scoped login; a refusal records
/// nothing.
fn admit(state: &AppState, config: &Config, identity: &AuthenticatedIdentity) -> Resolution {
    let resolver = match PrincipalResolver::from_config(config) {
        Ok(resolver) => resolver,
        Err(e) => {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"error": e.to_string()})),
                "ACP pairing: failed to compile the authorization policy; denying"
            );
            return Resolution::Denied;
        }
    };
    let resolved = match resolver.resolve(identity) {
        Ok(resolved) => resolved,
        Err(_deny) => return Resolution::VerifiedNotEntitled,
    };
    if entitled_to_nothing(&resolved.grants) {
        // A real identity (roster principal with no bound profile, or one
        // whose profiles name no agent) — verified, but there is nothing
        // on this daemon it may use.
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"principal": resolved.principal.display_id})),
            "ACP: credential verified but the principal is entitled to no agent; denying"
        );
        return Resolution::VerifiedNotEntitled;
    }
    // Admitted: an external (OIDC) subject is now "seen" on the ACP
    // surface too. Only the derived membership is stored, never the claims
    // or the bearer.
    if let Some(membership) = resolver.oidc_membership(identity) {
        state
            .external_subjects
            .record_login(identity, membership, resolved.grants.admin);
    }
    Resolution::Resolved(Box::new((resolved.principal, resolved.grants)))
}

/// A grant set that admits no agent at all: not admin and no
/// `allowed_agents` entry (the `*` wildcard counts as an entry).
#[must_use]
fn entitled_to_nothing(grants: &ResolvedGrants) -> bool {
    !grants.admin && grants.allowed_agents.is_empty()
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
        ConnAuth::Authenticated(auth) => *auth,
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

        let params = value.get("params").cloned().unwrap_or(Value::Null);
        let code = params.get("code").and_then(Value::as_str).unwrap_or("");

        match state.pairing.try_pair(code, client_id).await {
            Ok(Some(token)) => {
                // A principal-tagged code (`get-paircode --new --principal
                // <id>`) stashes a binding that must be drained into the live
                // runtime store BEFORE the token is resolved below — mirrors
                // the REST `/pair` handlers (`lib.rs::handle_pair`,
                // `api_pairing::submit_pairing_enhanced`) so a tagged code
                // works identically whether redeemed over REST or in-band.
                bind_pending_principal(state);

                // Record the device (with the optional `device_name` /
                // `device_type` params) the way REST `/api/pair` does, so an
                // in-band-paired client shows up named in the admin panel
                // instead of as a nameless legacy backfill at the next start.
                register_in_band_device(state, &token, &params, client_id);

                // Persist the new token to `gateway.paired_tokens` the same
                // way REST `/pair` and `/api/pair` do, so an in-band-paired
                // device survives a daemon restart (the paired set is
                // otherwise in-memory only). Logged, not fatal: the token is
                // live in-process either way, and this socket proceeds on it.
                if let Err(e) = super::persist_pairing_tokens(
                    state.config.clone(),
                    &state.pairing,
                    state.config_write_lock.clone(),
                )
                .await
                {
                    ::zeroclaw_log::record!(
                        ERROR,
                        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                            .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                            .with_attrs(::serde_json::json!({"error": e.to_string()})),
                        "in-band pairing succeeded but persisting the paired token to config failed; the token is active in-process only"
                    );
                }

                let authz_enforced = authz_enforced(&state.config.read());
                let principal_and_grants = match resolve_principal(state, Some(&token)).await {
                    Resolution::Resolved(principal_and_grants) => *principal_and_grants,
                    Resolution::Denied if !authz_enforced => {
                        (Principal::shared_operator(), ResolvedGrants::all())
                    }
                    Resolution::Denied | Resolution::VerifiedNotEntitled => {
                        // Paired, but the issued token is not entitled under
                        // enforced authz (not bound to any configured
                        // principal, or bound to one with no agent). Stay
                        // pre-auth rather than proceeding with no principal.
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

/// Insert an in-band-paired token into the device registry with the same
/// record REST `/api/pair` writes: optional `device_name` / `device_type`
/// from the pairing params (normalized by [`super::api_pairing::device_label`])
/// and the connection-derived client id as the address. No registry (pairing
/// not required) is a no-op. A failed insert is logged, not fatal, like the
/// token persistence next to it: the token is live in-process either way and
/// the startup backfill still surfaces it as a nameless legacy device.
fn register_in_band_device(state: &AppState, token: &str, params: &Value, client_id: &str) {
    use super::api_pairing::{
        DEVICE_NAME_MAX_CHARS, DEVICE_TYPE_MAX_CHARS, DeviceInfo, device_label,
    };

    let Some(registry) = state.device_registry.as_ref() else {
        return;
    };
    let now = chrono::Utc::now();
    if let Err(e) = registry.register(
        zeroclaw_config::pairing::PairingGuard::token_hash(token),
        DeviceInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: device_label(params.get("device_name"), DEVICE_NAME_MAX_CHARS),
            device_type: device_label(params.get("device_type"), DEVICE_TYPE_MAX_CHARS),
            paired_at: now,
            last_seen: now,
            ip_address: Some(client_id.to_string()),
            capabilities: None,
        },
    ) {
        ::zeroclaw_log::record!(
            ERROR,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": e.to_string()})),
            "in-band pairing succeeded but the device registry insert failed; the device will appear as a nameless legacy entry"
        );
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
mod provider_select_tests {
    //! The bearer-shape router: a JWT goes to the `oidc.<alias>` provider
    //! whose issuer it names and NEVER to the pairing provider; everything
    //! else is a pairing token.

    use super::*;
    use zeroclaw_api::principal::{AuthMethod, IdentitySubject, PrincipalId};
    use zeroclaw_config::schema::{AliasedAgentConfig, OidcConfig, PermissionProfileConfig};

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s)
    }

    fn jwt(header: &str, payload: &str) -> String {
        format!("{}.{}.{}", b64(header), b64(payload), b64("sig"))
    }

    fn config_with_issuers(entries: &[(&str, &str)]) -> Config {
        let mut config = Config::default();
        for (alias, issuer) in entries {
            config.oidc.insert(
                (*alias).to_owned(),
                OidcConfig {
                    issuer: (*issuer).to_owned(),
                    ..OidcConfig::default()
                },
            );
        }
        config
    }

    #[test]
    fn non_jwt_bearers_are_pairing_shaped() {
        for token in [
            "zc_0123456789abcdef",
            "a.b",
            "a..c",
            "..",
            "not.base64!.x",
            "x.y.z.w",
        ] {
            assert_eq!(classify_bearer(token), BearerShape::Pairing, "{token}");
        }
        // Three segments whose header is base64url but not JSON.
        assert_eq!(
            classify_bearer(&format!("{}.{}.{}", b64("hello"), b64("{}"), b64("s"))),
            BearerShape::Pairing
        );
    }

    #[test]
    fn jwt_shape_requires_an_alg_header() {
        assert_eq!(
            classify_bearer(&jwt(r#"{"typ":"JWT"}"#, r#"{"iss":"https://idp"}"#)),
            BearerShape::Pairing
        );
        assert_eq!(
            classify_bearer(&jwt(
                r#"{"alg":"ES256","kid":"k"}"#,
                r#"{"iss":"https://idp"}"#
            )),
            BearerShape::Jwt {
                issuer: Some("https://idp".to_owned())
            }
        );
        assert_eq!(
            classify_bearer(&jwt(r#"{"alg":"none"}"#, r#"{"sub":"x"}"#)),
            BearerShape::Jwt { issuer: None }
        );
    }

    #[test]
    fn pairing_tokens_and_absent_credentials_select_the_pairing_provider() {
        let config = config_with_issuers(&[("thunderbolt", "https://idp")]);
        assert_eq!(
            select_provider(&Credential::Bearer("zc_token".into()), &config).as_deref(),
            Some("pairing")
        );
        assert_eq!(
            select_provider(&Credential::None, &config).as_deref(),
            Some("pairing")
        );
    }

    #[test]
    fn jwt_selects_the_alias_whose_issuer_it_names() {
        let config =
            config_with_issuers(&[("corp", "https://corp"), ("thunderbolt", "https://idp")]);
        let token = jwt(
            r#"{"alg":"ES256"}"#,
            r#"{"iss":"https://idp","sub":"alice"}"#,
        );
        assert_eq!(
            select_provider(&Credential::Bearer(token), &config).as_deref(),
            Some("oidc.thunderbolt")
        );
    }

    #[test]
    fn jwt_for_an_untrusted_issuer_selects_nothing_and_never_falls_back() {
        let config = config_with_issuers(&[("thunderbolt", "https://idp")]);
        let foreign = jwt(r#"{"alg":"ES256"}"#, r#"{"iss":"https://evil"}"#);
        assert_eq!(select_provider(&Credential::Bearer(foreign), &config), None);
        let no_iss = jwt(r#"{"alg":"ES256"}"#, r#"{"sub":"alice"}"#);
        assert_eq!(select_provider(&Credential::Bearer(no_iss), &config), None);
        // No OIDC trust configured at all: a JWT is still not a pairing token.
        let token = jwt(r#"{"alg":"ES256"}"#, r#"{"iss":"https://idp"}"#);
        assert_eq!(
            select_provider(&Credential::Bearer(token), &Config::default()),
            None
        );
    }

    #[test]
    fn duplicate_issuers_resolve_deterministically_by_alias_order() {
        let config = config_with_issuers(&[("zeta", "https://idp"), ("alpha", "https://idp")]);
        let token = jwt(r#"{"alg":"ES256"}"#, r#"{"iss":"https://idp"}"#);
        assert_eq!(
            select_provider(&Credential::Bearer(token), &config).as_deref(),
            Some("oidc.alpha")
        );
    }

    #[test]
    fn subprotocol_echo_prefers_volt_then_zeroclaw_and_keeps_bearer_entries_out() {
        assert_eq!(select_acp_subprotocol(None), None);
        assert_eq!(select_acp_subprotocol(Some("")), None);
        assert_eq!(select_acp_subprotocol(Some("bearer.zc_tok")), None);
        assert_eq!(
            select_acp_subprotocol(Some("volt.acp.v1")),
            Some("volt.acp.v1")
        );
        assert_eq!(
            select_acp_subprotocol(Some("zeroclaw.acp.v1")),
            Some("zeroclaw.acp.v1")
        );
        // Both offered: volt wins regardless of the client's order.
        assert_eq!(
            select_acp_subprotocol(Some("zeroclaw.acp.v1, volt.acp.v1")),
            Some("volt.acp.v1")
        );
        assert_eq!(
            select_acp_subprotocol(Some("volt.acp.v1,zeroclaw.acp.v1")),
            Some("volt.acp.v1")
        );
        // The frozen client shape: subprotocol + bearer entry, any order.
        assert_eq!(
            select_acp_subprotocol(Some("volt.acp.v1, bearer.zc_tok")),
            Some("volt.acp.v1")
        );
        assert_eq!(
            select_acp_subprotocol(Some("bearer.zc_tok, zeroclaw.acp.v1")),
            Some("zeroclaw.acp.v1")
        );
        // Unknown names are never echoed.
        assert_eq!(select_acp_subprotocol(Some("acp.v2, volt.acp.v2")), None);
    }

    #[test]
    fn ws_token_is_extracted_from_the_bearer_subprotocol_entry() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::SEC_WEBSOCKET_PROTOCOL,
            "volt.acp.v1, bearer.zc_tok".parse().unwrap(),
        );
        assert_eq!(extract_ws_token(&headers, None), Some("zc_tok"));
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::SEC_WEBSOCKET_PROTOCOL,
            "bearer.zc_tok,zeroclaw.acp.v1".parse().unwrap(),
        );
        assert_eq!(extract_ws_token(&headers, Some("ignored")), Some("zc_tok"));
    }

    #[test]
    fn oidc_trust_alone_turns_enforcement_on() {
        assert!(!authz_enforced(&Config::default()));
        assert!(authz_enforced(&config_with_issuers(&[(
            "thunderbolt",
            "https://idp"
        )])));
    }

    /// `[oidc.thunderbolt]` trusting `https://idp`, mapping the `ops`
    /// group to the `crm` profile (entitled to `crm-bot`).
    fn oidc_state() -> AppState {
        let mut config = config_with_issuers(&[("thunderbolt", "https://idp")]);
        let oidc = config.oidc.get_mut("thunderbolt").unwrap();
        // `validate_auth` (run by `PrincipalResolver::from_config`) requires
        // an audience; without one every admission fails closed as Denied.
        oidc.audience = "zeroclaw".to_owned();
        oidc.claim_path = "groups".to_owned();
        oidc.profile_map.insert("ops".to_owned(), "crm".to_owned());
        config
            .agents
            .insert("crm-bot".to_owned(), AliasedAgentConfig::default());
        config.permission_profiles.insert(
            "crm".to_owned(),
            PermissionProfileConfig {
                allowed_agents: vec!["crm-bot".to_owned()],
                ..PermissionProfileConfig::default()
            },
        );
        crate::api::tests::test_state(config)
    }

    /// What the `oidc.thunderbolt` provider hands back for alice once her
    /// JWT verified, carrying `groups`.
    fn verified_oidc(groups: &[&str]) -> AuthenticatedIdentity {
        let mut claims = serde_json::Map::new();
        claims.insert("groups".to_owned(), serde_json::json!(groups));
        AuthenticatedIdentity::new(
            IdentitySubject::Oidc {
                issuer: "https://idp".to_owned(),
                subject: "alice".to_owned(),
            },
            AuthMethod::Oidc,
        )
        .with_provider_alias("thunderbolt")
        .with_claims(claims)
    }

    #[test]
    fn admitted_oidc_identity_is_recorded_as_an_external_subject() {
        let state = oidc_state();
        let config = state.config.read().clone();
        assert!(state.external_subjects.list().is_empty());
        for _ in 0..2 {
            assert!(matches!(
                admit(&state, &config, &verified_oidc(&["ops", "unmapped"])),
                Resolution::Resolved(_)
            ));
        }
        let seen = state.external_subjects.list();
        assert_eq!(seen.len(), 1, "one subject, upserted");
        let record = &seen[0];
        assert_eq!(record.id, PrincipalId::for_oidc("https://idp", "alice").0);
        assert_eq!(record.provider, "thunderbolt");
        assert_eq!(record.issuer, "https://idp");
        assert_eq!(record.subject, "alice");
        assert_eq!(record.groups, vec!["ops", "unmapped"]);
        assert_eq!(record.profiles, vec!["crm"]);
        assert!(!record.admin);
        assert_eq!(record.logins, 2);
    }

    #[test]
    fn refused_oidc_identity_is_not_recorded() {
        let state = oidc_state();
        let config = state.config.read().clone();
        assert!(matches!(
            admit(&state, &config, &verified_oidc(&["unmapped"])),
            Resolution::VerifiedNotEntitled
        ));
        assert!(state.external_subjects.list().is_empty());
    }
}

#[cfg(test)]
mod revocation_tests {
    //! Revocation must cut off a token even when a
    //! `[[authz.principals]].token_hashes` pin or a `TokenBindingStore`
    //! binding still names it: `resolve_principal` requires pairing
    //! liveness before the pairing provider ever sees the bearer, and every
    //! revoke path drops the binding too.

    use super::*;
    use zeroclaw_config::authz::{PrincipalRecord, TokenBindingStore};
    use zeroclaw_config::pairing::{PairingCodePolicy, PairingGuard};
    use zeroclaw_config::schema::{AliasedAgentConfig, PermissionProfileConfig};

    /// `require_pairing = true`, `paired` as the live paired set, `alice`
    /// configured with one entitled agent (`crm-bot`, via the `crm`
    /// profile), `bound` pre-seeded into the live binding store, `pinned`
    /// as alice's `token_hashes`.
    fn state_with(paired: &[&str], bound: &[&str], pinned: &[&str]) -> AppState {
        state_with_profiles(paired, bound, pinned, &["crm"])
    }

    /// As [`state_with`], with alice bound to `profiles` (any subset of the
    /// configured `crm` profile — `&[]` leaves her verified but entitled
    /// to nothing).
    fn state_with_profiles(
        paired: &[&str],
        bound: &[&str],
        pinned: &[&str],
        profiles: &[&str],
    ) -> AppState {
        let mut config = Config::default();
        config.gateway.require_pairing = true;
        // `validate_auth` rejects a profile naming an agent that is not
        // configured, so the entitled agent must exist.
        config
            .agents
            .insert("crm-bot".to_string(), AliasedAgentConfig::default());
        config.permission_profiles.insert(
            "crm".to_string(),
            PermissionProfileConfig {
                allowed_agents: vec!["crm-bot".to_string()],
                ..PermissionProfileConfig::default()
            },
        );
        config.authz.principals.push(PrincipalRecord {
            id: "alice".to_string(),
            token_hashes: pinned.iter().map(|t| PairingGuard::token_hash(t)).collect(),
            profiles: profiles.iter().map(|p| (*p).to_string()).collect(),
        });
        let mut state = crate::api::tests::test_state(config);
        let paired: Vec<String> = paired.iter().map(|t| (*t).to_string()).collect();
        state.pairing = Arc::new(PairingGuard::new(
            true,
            &paired,
            PairingCodePolicy::default(),
        ));
        let bindings = TokenBindingStore::new_ephemeral();
        for token in bound {
            bindings
                .set(PairingGuard::token_hash(token), "alice".to_string())
                .unwrap();
        }
        state.token_bindings = Arc::new(bindings);
        state
    }

    #[test]
    fn liveness_requires_a_paired_token_only_when_pairing_is_required() {
        assert!(pairing_bearer_is_live(false, false));
        assert!(pairing_bearer_is_live(false, true));
        assert!(!pairing_bearer_is_live(true, false));
        assert!(pairing_bearer_is_live(true, true));
    }

    #[tokio::test]
    async fn bound_but_unpaired_token_is_denied() {
        // The binding store still names the token (revocation never
        // reached it, or the operator edited `gateway.paired_tokens` by
        // hand) — the token is NOT in the paired set, so it must not
        // resolve.
        let state = state_with(&[], &["zc_bound"], &[]);
        assert!(matches!(
            resolve_principal(&state, Some("zc_bound")).await,
            Resolution::Denied
        ));
    }

    #[tokio::test]
    async fn pinned_but_unpaired_token_is_denied() {
        let state = state_with(&[], &[], &["zc_pinned"]);
        assert!(matches!(
            resolve_principal(&state, Some("zc_pinned")).await,
            Resolution::Denied
        ));
    }

    #[tokio::test]
    async fn bound_and_paired_token_resolves_to_its_principal() {
        let state = state_with(&["zc_bound"], &["zc_bound"], &[]);
        let Resolution::Resolved(resolved) = resolve_principal(&state, Some("zc_bound")).await
        else {
            panic!("a paired AND bound token resolves");
        };
        let (principal, grants) = *resolved;
        assert!(principal.is_authenticated(), "a distinct Roster principal");
        assert_eq!(principal.display_id, "alice");
        assert!(grants.may_use_agent("crm-bot"));
    }

    #[tokio::test]
    async fn revoking_the_pairing_cuts_off_a_bound_token() {
        let state = state_with(&["zc_bound"], &["zc_bound"], &[]);
        assert!(matches!(
            resolve_principal(&state, Some("zc_bound")).await,
            Resolution::Resolved(_)
        ));
        let hash = PairingGuard::token_hash("zc_bound");
        assert!(state.pairing.revoke_token_hash(&hash));
        assert!(
            matches!(
                resolve_principal(&state, Some("zc_bound")).await,
                Resolution::Denied
            ),
            "a revoked token must stop resolving even though its binding still exists"
        );
        unbind_token_hash(&state, &hash);
        assert!(state.token_bindings.get(&hash).is_none());
    }

    /// A paired, bound token whose principal holds no profile (or only
    /// profiles naming no agent) is VERIFIED but entitled to nothing —
    /// distinct from `Denied`, so the upgrade can answer `401` instead of
    /// pre-auth or an empty-roster socket.
    #[tokio::test]
    async fn paired_and_bound_principal_with_no_agents_is_verified_not_entitled() {
        let state = state_with_profiles(&["zc_bound"], &["zc_bound"], &[], &[]);
        assert!(matches!(
            resolve_principal(&state, Some("zc_bound")).await,
            Resolution::VerifiedNotEntitled
        ));
    }

    /// Paired but neither bound nor pinned under enforced authz: the
    /// provider never verified it — `Denied`, not `VerifiedNotEntitled`.
    #[tokio::test]
    async fn paired_but_unbound_token_is_denied_not_verified() {
        let state = state_with(&["zc_stranger"], &[], &[]);
        assert!(matches!(
            resolve_principal(&state, Some("zc_stranger")).await,
            Resolution::Denied
        ));
    }

    #[tokio::test]
    async fn absent_token_is_denied() {
        let state = state_with(&[], &[], &[]);
        assert!(matches!(
            resolve_principal(&state, None).await,
            Resolution::Denied
        ));
        assert!(matches!(
            resolve_principal(&state, Some("")).await,
            Resolution::Denied
        ));
    }

    #[test]
    fn unbind_all_tokens_clears_every_binding() {
        let state = state_with(&["a", "b"], &["a", "b"], &[]);
        assert!(
            state
                .token_bindings
                .get(&PairingGuard::token_hash("a"))
                .is_some()
        );
        state.pairing.revoke_all_tokens();
        unbind_all_tokens(&state);
        assert!(
            state
                .token_bindings
                .get(&PairingGuard::token_hash("a"))
                .is_none()
        );
        assert!(
            state
                .token_bindings
                .get(&PairingGuard::token_hash("b"))
                .is_none()
        );
    }

    /// `state_with` plus a fresh on-disk device registry (what
    /// `run_gateway` builds when pairing is required). Returns the tempdir
    /// so the registry's SQLite file outlives the test body.
    fn state_with_registry() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = state_with(&[], &[], &[]);
        state.device_registry = Some(Arc::new(crate::api_pairing::DeviceRegistry::new(
            tmp.path(),
        )));
        (state, tmp)
    }

    fn registered_devices(state: &AppState) -> Vec<crate::api_pairing::DeviceInfo> {
        state.device_registry.as_ref().unwrap().list().unwrap()
    }

    #[test]
    fn in_band_pair_registers_device_name_and_type() {
        let (state, _tmp) = state_with_registry();
        register_in_band_device(
            &state,
            "zc_in_band",
            &serde_json::json!({
                "code": "ignored",
                "device_name": "  Volt Admin\u{0007} (Dave)\n",
                "device_type": "desktop",
            }),
            "203.0.113.7",
        );

        let devices = registered_devices(&state);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name.as_deref(), Some("Volt Admin (Dave)"));
        assert_eq!(devices[0].device_type.as_deref(), Some("desktop"));
        assert_eq!(devices[0].ip_address.as_deref(), Some("203.0.113.7"));
        // The row is keyed by the token's hash, same as the REST handlers.
        assert!(
            state
                .device_registry
                .as_ref()
                .unwrap()
                .update_capabilities(&PairingGuard::token_hash("zc_in_band"), vec![])
        );
    }

    #[test]
    fn in_band_pair_without_labels_keeps_name_none() {
        let (state, _tmp) = state_with_registry();
        register_in_band_device(
            &state,
            "zc_in_band",
            &serde_json::json!({ "code": "ignored" }),
            "203.0.113.7",
        );

        let devices = registered_devices(&state);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, None);
        assert_eq!(devices[0].device_type, None);
    }

    #[test]
    fn in_band_pair_truncates_over_long_labels() {
        let (state, _tmp) = state_with_registry();
        let long_name = "n".repeat(500);
        let long_type = "t".repeat(100);
        register_in_band_device(
            &state,
            "zc_in_band",
            &serde_json::json!({ "device_name": long_name, "device_type": long_type }),
            "203.0.113.7",
        );

        let devices = registered_devices(&state);
        assert_eq!(
            devices[0].name.as_deref(),
            Some(
                "n".repeat(crate::api_pairing::DEVICE_NAME_MAX_CHARS)
                    .as_str()
            )
        );
        assert_eq!(
            devices[0].device_type.as_deref(),
            Some(
                "t".repeat(crate::api_pairing::DEVICE_TYPE_MAX_CHARS)
                    .as_str()
            )
        );
    }

    #[test]
    fn in_band_pair_without_registry_is_a_no_op() {
        let state = state_with(&[], &[], &[]);
        assert!(state.device_registry.is_none());
        register_in_band_device(
            &state,
            "zc_in_band",
            &serde_json::json!({ "device_name": "x" }),
            "203.0.113.7",
        );
    }
}

#[cfg(test)]
mod upgrade_decision_tests {
    //! The pure `/acp` upgrade decision behind the frozen contract:
    //! verified-but-unentitled ⇒ 401 (never pre-auth, never the shared
    //! operator); denied ⇒ pre-auth when pairing is required and unmet,
    //! else legacy fallback when authz is not enforced, else 401.

    use super::*;
    use zeroclaw_api::principal::{
        AuthMethod, AuthenticatedIdentity, IdentitySubject, PrincipalId,
    };

    fn distinct() -> Resolution {
        let identity = AuthenticatedIdentity::new(
            IdentitySubject::Roster {
                principal_id: "alice".to_string(),
            },
            AuthMethod::Native,
        );
        let mut grants = ResolvedGrants::none();
        grants.allowed_agents = vec![zeroclaw_api::principal::AgentAlias("crm-bot".to_string())];
        Resolution::Resolved(Box::new((Principal::from_identity(&identity), grants)))
    }

    fn is_unauthorized(decision: &UpgradeDecision) -> bool {
        matches!(decision, UpgradeDecision::Unauthorized)
    }

    fn is_pre_auth(decision: &UpgradeDecision) -> bool {
        matches!(decision, UpgradeDecision::PreAuth)
    }

    fn authenticated_as(decision: UpgradeDecision) -> (Principal, ResolvedGrants) {
        match decision {
            UpgradeDecision::Authenticated(principal_and_grants) => *principal_and_grants,
            UpgradeDecision::PreAuth => panic!("expected Authenticated, got PreAuth"),
            UpgradeDecision::Unauthorized => panic!("expected Authenticated, got Unauthorized"),
        }
    }

    #[test]
    fn verified_not_entitled_is_401_in_every_mode() {
        for require_pairing in [false, true] {
            for token_is_paired in [false, true] {
                for authz_enforced in [false, true] {
                    let decision = upgrade_decision(
                        Resolution::VerifiedNotEntitled,
                        require_pairing,
                        token_is_paired,
                        authz_enforced,
                    );
                    assert!(
                        is_unauthorized(&decision),
                        "require_pairing={require_pairing} paired={token_is_paired} enforced={authz_enforced}"
                    );
                }
            }
        }
    }

    #[test]
    fn denied_with_pairing_required_and_unmet_is_pre_auth() {
        assert!(is_pre_auth(&upgrade_decision(
            Resolution::Denied,
            true,
            false,
            false
        )));
        assert!(is_pre_auth(&upgrade_decision(
            Resolution::Denied,
            true,
            false,
            true
        )));
    }

    #[test]
    fn denied_but_paired_is_401_when_enforced_and_shared_operator_otherwise() {
        assert!(is_unauthorized(&upgrade_decision(
            Resolution::Denied,
            true,
            true,
            true
        )));
        let (principal, grants) =
            authenticated_as(upgrade_decision(Resolution::Denied, true, true, false));
        assert_eq!(principal.id, PrincipalId::shared_operator());
        assert!(grants.admin);
    }

    #[test]
    fn denied_with_pairing_disabled_follows_enforcement() {
        assert!(is_unauthorized(&upgrade_decision(
            Resolution::Denied,
            false,
            false,
            true
        )));
        let (principal, _) =
            authenticated_as(upgrade_decision(Resolution::Denied, false, false, false));
        assert!(!principal.is_authenticated());
    }

    #[test]
    fn resolved_distinct_principal_is_authenticated_even_when_unpaired() {
        // Condition 3 of `connection_is_authenticated`: a config-backed
        // distinct principal satisfies the pairing gate on its own.
        let (principal, grants) = authenticated_as(upgrade_decision(distinct(), true, false, true));
        assert!(principal.is_authenticated());
        assert!(grants.may_use_agent("crm-bot"));
        assert!(!grants.admin, "a distinct principal keeps its OWN grants");
    }

    #[test]
    fn resolved_shared_operator_cannot_satisfy_the_pairing_gate_alone() {
        let shared = Resolution::Resolved(Box::new((
            Principal::shared_operator(),
            ResolvedGrants::all(),
        )));
        assert!(is_pre_auth(&upgrade_decision(shared, true, false, false)));
    }

    #[test]
    fn entitled_to_nothing_is_no_admin_and_no_agent() {
        assert!(entitled_to_nothing(&ResolvedGrants::none()));
        assert!(!entitled_to_nothing(&ResolvedGrants::all()));
        let mut grants = ResolvedGrants::none();
        grants.allowed_agents = vec![zeroclaw_api::principal::AgentAlias("x".to_string())];
        assert!(!entitled_to_nothing(&grants));
    }
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
        cfg.authz
            .principals
            .push(zeroclaw_config::authz::PrincipalRecord {
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
