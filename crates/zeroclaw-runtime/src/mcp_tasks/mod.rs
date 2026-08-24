//! MCP task supervisor: dispatches long-running MCP tool calls (the
//! `io.modelcontextprotocol/tasks` extension) to a per-task background
//! poller, and hands a completed task's result off for delivery into the
//! owning conversation once the task reaches a terminal state.
//!
//! This module owns admission control (a per-scope concurrent-task cap),
//! lazily-built per-agent-scope [`McpRegistry`] connections that advertise
//! the tasks extension, and the poll loop itself. Actually injecting a
//! completed task's result back into the agent loop is a separate concern
//! (Task 6's `mcp_tasks::inject` submodule) — this module never calls into
//! it directly. Doing so would create a compile-time dependency cycle
//! between the two halves of the feature, since `inject` needs types from
//! here (`TaskBinding`). Instead, the poller depends only on the
//! [`TaskInjector`] trait; `inject` provides the production implementation,
//! and tests provide a fake.

pub(crate) mod inject;
pub(crate) mod wrapper;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use zeroclaw_config::schema::Config;
use zeroclaw_tools::embedded_resource::format_mcp_tool_result_for_model;
use zeroclaw_tools::mcp_client::TaskCall;
use zeroclaw_tools::mcp_protocol::{GetTaskResult, MODEL_IMMEDIATE_RESPONSE_KEY};

use crate::tools::McpRegistry;

/// Outcome of [`McpTaskSupervisor::create_task`].
pub(crate) enum TaskDispatch {
    /// A task was created and is now being polled in the background.
    /// `immediate` is the text to surface to the model right now — either a
    /// server-provided immediate response (the
    /// `io.modelcontextprotocol/model-immediate-response` meta key) or a
    /// generic placeholder. The eventual result is delivered later, out of
    /// band, via [`TaskInjector::inject`].
    Pending { immediate: String },
    /// The tool call ran to completion inline (the server either does not
    /// support the tasks extension for this tool, or chose to run
    /// synchronously). Carries the pretty-printed JSON result.
    Inline(String),
}

/// Binds a live polled task back to the scope and session that created it.
/// Read by the poll loop below and by [`McpTaskSupervisor::cancel_tasks_for_session`].
/// Both the struct and its fields are `pub(crate)` — the struct because it
/// appears in the `pub trait TaskInjector`'s method signature (a `pub` item
/// cannot expose a strictly module-private type), the fields so Task 6's
/// `mcp_tasks::inject` submodule can read `session_key` / `agent_alias` /
/// `server` when routing a completed task's result into the right
/// conversation.
pub(crate) struct TaskBinding {
    pub(crate) session_key: Option<String>,
    pub(crate) agent_alias: String,
    pub(crate) server: String,
    /// Last known poll interval for this task. Not currently re-read after
    /// creation (the poll loop tracks its own local cadence), kept for
    /// Task 6 / future bookkeeping.
    #[allow(dead_code)]
    pub(crate) poll_interval_ms: u64,
    /// Origin channel name (e.g. `"telegram"`) the tool call that created
    /// this task ran under, captured from `TOOL_LOOP_ORIGIN_ROUTE` at
    /// creation time. `None` when the call did not originate from a channel
    /// turn (CLI/cron/gateway/subagent) — there is then nowhere to deliver
    /// the completion back to besides memory.
    pub(crate) channel: Option<String>,
    /// Origin reply target (channel-specific recipient/thread id) paired
    /// with `channel` above.
    pub(crate) reply_target: Option<String>,
}

/// Delivers a completed (or failed/cancelled) task's result back into its
/// owning conversation. Implemented by `mcp_tasks::inject`'s production
/// injector (Task 6); the poller depends only on this trait so the two
/// halves of the feature can be built independently without a circular
/// module dependency.
#[async_trait::async_trait]
pub(crate) trait TaskInjector: Send + Sync {
    async fn inject(&self, binding: TaskBinding, got: GetTaskResult);
}

/// Owns per-agent-scope task-advertising MCP connections, admission control,
/// and the background pollers for every in-flight task.
///
/// The struct is `pub` (Ruling R10) because [`crate::tools::scoped::ScopedAssembly`]
/// carries `Option<Arc<McpTaskSupervisor>>` as a `pub` field and that struct
/// is constructed across crate boundaries (`zeroclaw-channels`). Most of what's
/// here - the methods and the [`TaskDispatch`]/[`TaskInjector`]/[`TaskBinding`]
/// types - stays `pub(crate)`: only `assemble` needs to hold and clone the
/// `Arc`, never to call into it directly from outside this crate.
/// [`Self::cancel_tasks_for_session`] is the one exception, `pub` since Task 9's
/// session-interruption handler (`zeroclaw-channels::orchestrator`) calls it
/// directly on the shared `Arc` it already holds.
pub struct McpTaskSupervisor {
    /// Kept solely for the per-scope, per-server admission cap lookup in
    /// [`Self::create_task`] (`mcp_servers_for_agent` /
    /// `task_concurrency_cap`) — registry resolution itself goes through
    /// [`Self::pool`], not this snapshot.
    config: Config,
    /// The daemon-owned, shared MCP connection pool this supervisor borrows
    /// registries from (`pool.registry_for(alias)`), so the task poller
    /// polls the exact same pooled connection an agent turn in that scope
    /// uses, instead of maintaining a duplicate per-scope registry here.
    pool: Arc<crate::mcp_pool::McpConnectionPool>,
    tasks: Mutex<HashMap<String, TaskBinding>>,
    injector: Arc<dyn TaskInjector>,
    /// Test seam for [`McpTaskToolWrapper`](wrapper::McpTaskToolWrapper)'s unit
    /// test: when set, `create_task` short-circuits admission/registry/poll
    /// entirely and returns `TaskDispatch::Pending { immediate }` verbatim,
    /// recording the `session_key` it was called with into
    /// [`Self::last_session_key`]. Never set outside tests.
    #[cfg(test)]
    test_pending: Option<String>,
    #[cfg(test)]
    last_session_key: std::sync::Mutex<Option<String>>,
    /// Test seam for `mcp_tasks::wrapper`'s unit test: the
    /// `(channel, reply_target)` most recently passed to `create_task`,
    /// paired with [`Self::last_session_key`]. Confirms
    /// `McpTaskToolWrapper::execute` correctly threads
    /// `TOOL_LOOP_ORIGIN_ROUTE` through to `create_task` (Task 8b).
    #[cfg(test)]
    last_origin_route: std::sync::Mutex<Option<(Option<String>, Option<String>)>>,
    /// Test seam for [`Self::cancel_tasks_for_session`]'s unit test: every
    /// task id it attempts to cancel (i.e. every victim it iterates, whether
    /// or not the best-effort registry `tasks/cancel` call itself succeeds)
    /// is recorded here. Read via [`Self::cancel_calls`]. Never populated
    /// outside tests.
    #[cfg(test)]
    cancel_calls: std::sync::Mutex<Vec<String>>,
}

impl McpTaskSupervisor {
    /// Production constructor: builds a supervisor wired to the real
    /// [`inject::RuntimeInjector`] (reactive `agent::run` delivery). This is
    /// the one `pub` entry point — every cross-crate wiring site (the
    /// daemon's `DaemonRegistry`, `zeroclaw-channels::start_channels`,
    /// `zeroclaw-gateway`'s `AppState`) constructs its shared supervisor
    /// through this, since [`Self::new`] takes an arbitrary injector (a test
    /// seam) and `RuntimeInjector` itself is `pub(crate)`. Call once per
    /// daemon run/reload iteration and share the returned `Arc`; a fresh
    /// call per surface would connect duplicate MCP task-advertising
    /// registries for the same agent scope.
    pub fn start(config: Config, pool: Arc<crate::mcp_pool::McpConnectionPool>) -> Arc<Self> {
        let injector: Arc<dyn TaskInjector> = Arc::new(inject::RuntimeInjector {
            config: config.clone(),
        });
        Self::new(config, injector, pool)
    }

    pub(crate) fn new(
        config: Config,
        injector: Arc<dyn TaskInjector>,
        pool: Arc<crate::mcp_pool::McpConnectionPool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            pool,
            tasks: Mutex::new(HashMap::new()),
            injector,
            #[cfg(test)]
            test_pending: None,
            #[cfg(test)]
            last_session_key: std::sync::Mutex::new(None),
            #[cfg(test)]
            last_origin_route: std::sync::Mutex::new(None),
            #[cfg(test)]
            cancel_calls: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Test-only: build a supervisor whose `alias` scope always resolves
    /// (via `pool.registry_for`) to `registry`, so `create_task` never has
    /// to connect a real MCP server. Lets the poller logic (admission,
    /// polling, terminal-state injection) be exercised end to end against a
    /// fake registry.
    #[cfg(test)]
    fn new_for_test(
        alias: &str,
        registry: Arc<McpRegistry>,
        injector: Arc<dyn TaskInjector>,
    ) -> Arc<Self> {
        let pool = crate::mcp_pool::McpConnectionPool::for_test_with_registry(alias, registry);
        Arc::new(Self {
            config: Config::default(),
            pool,
            tasks: Mutex::new(HashMap::new()),
            injector,
            test_pending: None,
            last_session_key: std::sync::Mutex::new(None),
            last_origin_route: std::sync::Mutex::new(None),
            cancel_calls: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Test-only seam for `mcp_tasks::wrapper`'s unit test: a supervisor
    /// that never connects a real MCP registry and always answers
    /// `create_task` with `TaskDispatch::Pending { immediate }`, recording
    /// the `session_key` it was called with so the test can assert the
    /// wrapper correctly threaded the `TOOL_LOOP_SESSION_KEY` task-local
    /// through to `create_task`.
    #[cfg(test)]
    pub(crate) fn new_for_test_pending(immediate: &str) -> Arc<Self> {
        struct NoopInjector;
        #[async_trait::async_trait]
        impl TaskInjector for NoopInjector {
            async fn inject(&self, _binding: TaskBinding, _got: GetTaskResult) {}
        }
        Arc::new(Self {
            config: Config::default(),
            pool: crate::mcp_pool::McpConnectionPool::from_owned_config(Config::default()),
            tasks: Mutex::new(HashMap::new()),
            injector: Arc::new(NoopInjector),
            test_pending: Some(immediate.to_string()),
            last_session_key: std::sync::Mutex::new(None),
            last_origin_route: std::sync::Mutex::new(None),
            cancel_calls: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Test-only seam for [`Self::cancel_tasks_for_session`]'s unit test: a
    /// supervisor pre-populated with one live task (`task_id`, on `server`,
    /// scoped to `alias`) bound to `session_key`, with no MCP servers
    /// configured for `alias` (a default `Config`-backed pool) — so the
    /// best-effort registry cancel inside `cancel_tasks_for_session` finds
    /// `pool.registry_for(&alias)` returns `None` and is silently discarded
    /// exactly as it is in production against a server that has since
    /// disconnected. What the unit test actually asserts is
    /// [`Self::cancel_calls`] (recorded unconditionally, before the
    /// registry round trip) and [`Self::is_empty`] (the binding is dropped
    /// either way).
    #[cfg(test)]
    pub(crate) fn new_for_test_with_live_task(
        session_key: &str,
        alias: &str,
        task_id: &str,
    ) -> Arc<Self> {
        struct NoopInjector;
        #[async_trait::async_trait]
        impl TaskInjector for NoopInjector {
            async fn inject(&self, _binding: TaskBinding, _got: GetTaskResult) {}
        }
        let mut tasks = HashMap::new();
        tasks.insert(
            task_id.to_string(),
            TaskBinding {
                session_key: Some(session_key.to_string()),
                agent_alias: alias.to_string(),
                server: alias.to_string(),
                poll_interval_ms: 1000,
                channel: None,
                reply_target: None,
            },
        );
        Arc::new(Self {
            config: Config::default(),
            pool: crate::mcp_pool::McpConnectionPool::from_owned_config(Config::default()),
            tasks: Mutex::new(tasks),
            injector: Arc::new(NoopInjector),
            test_pending: None,
            last_session_key: std::sync::Mutex::new(None),
            last_origin_route: std::sync::Mutex::new(None),
            cancel_calls: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// The `session_key` most recently passed to `create_task`. Test seam,
    /// paired with [`Self::new_for_test_pending`].
    #[cfg(test)]
    pub(crate) fn last_session_key(&self) -> Option<String> {
        self.last_session_key.lock().unwrap().clone()
    }

    /// The `(channel, reply_target)` most recently passed to `create_task`.
    /// Test seam, paired with [`Self::new_for_test_pending`].
    #[cfg(test)]
    pub(crate) fn last_origin_route(&self) -> Option<(Option<String>, Option<String>)> {
        self.last_origin_route.lock().unwrap().clone()
    }

    /// Every task id [`Self::cancel_tasks_for_session`] has attempted to
    /// cancel so far. Test seam, paired with
    /// [`Self::new_for_test_with_live_task`].
    #[cfg(test)]
    pub(crate) fn cancel_calls(&self) -> Vec<String> {
        self.cancel_calls.lock().unwrap().clone()
    }

    /// Whether the live-task table is empty. Test seam, paired with
    /// [`Self::new_for_test_with_live_task`].
    #[cfg(test)]
    pub(crate) async fn is_empty(&self) -> bool {
        self.tasks.lock().await.is_empty()
    }

    /// Create a task for `alias` on `server`/`tool`. Subject to a
    /// per-scope, per-server admission cap
    /// (`McpServerConfig::task_concurrency_cap`); a call over the cap is
    /// rejected inline rather than queued.
    pub(crate) async fn create_task(
        self: &Arc<Self>,
        alias: &str,
        server: &str,
        tool: &str,
        args: serde_json::Value,
        session_key: Option<String>,
        channel: Option<String>,
        reply_target: Option<String>,
    ) -> anyhow::Result<TaskDispatch> {
        #[cfg(test)]
        if let Some(immediate) = self.test_pending.clone() {
            *self.last_session_key.lock().unwrap() = session_key;
            *self.last_origin_route.lock().unwrap() = Some((channel, reply_target));
            return Ok(TaskDispatch::Pending { immediate });
        }

        // Admission cap per scope.
        let cap = self
            .config
            .mcp_servers_for_agent(alias)
            .iter()
            .find(|s| s.name == server)
            .map(|s| s.task_concurrency_cap())
            .unwrap_or(32);
        {
            let tasks = self.tasks.lock().await;
            let live = tasks.values().filter(|b| b.agent_alias == alias).count() as u32;
            if live >= cap {
                return Ok(TaskDispatch::Inline(format!(
                    "Task rejected: {live} tasks already active for this agent (cap {cap})."
                )));
            }
        }

        let reg = self
            .pool
            .registry_for(alias)
            .await
            .ok_or_else(|| anyhow::anyhow!("no MCP servers for scope `{alias}`"))?;
        let prefixed = format!("{server}__{tool}");
        match reg.create_task(&prefixed, args).await? {
            TaskCall::Inline(v) => {
                // A server that does not implement the tasks extension (or that
                // runs this call synchronously) answers inline. Its result may
                // carry base64 resource/image/audio payloads, so materialize
                // them to the agent's workspace and hand the model markers —
                // exactly the intake the inline `McpToolWrapper` path applies.
                // Serializing the raw result here (the pre-fix behavior) floods
                // the model's context and trips a `context_window` failure.
                let workspace_dir = self.config.agent_workspace_dir(alias);
                let text = format_mcp_tool_result_for_model(v.clone(), &workspace_dir)
                    .unwrap_or_else(|_| serde_json::to_string_pretty(&v).unwrap_or_default());
                Ok(TaskDispatch::Inline(text))
            }
            TaskCall::Task(ct) => {
                let poll = ct.task.poll_interval_ms.unwrap_or(1000);
                let immediate = ct
                    .meta
                    .as_ref()
                    .and_then(|m| m.get(MODEL_IMMEDIATE_RESPONSE_KEY))
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        format!(
                            "Task started (id={}). I'll report the result when it completes.",
                            ct.task.task_id
                        )
                    });
                self.tasks.lock().await.insert(
                    ct.task.task_id.clone(),
                    TaskBinding {
                        session_key,
                        agent_alias: alias.to_string(),
                        server: server.to_string(),
                        poll_interval_ms: poll,
                        channel,
                        reply_target,
                    },
                );
                self.clone()
                    .spawn_poller(ct.task.task_id.clone(), poll, Arc::clone(&reg));
                Ok(TaskDispatch::Pending { immediate })
            }
        }
    }

    /// Background poll loop for one task. Polls on the exact registry
    /// (`reg`) the task was created on — NOT a freshly re-fetched
    /// `pool.registry_for(alias)` — because rmcp task ids are scoped to the
    /// MCP connection that created them. Re-fetching here would race a
    /// concurrent config change reconciled by the pool: a server
    /// reconnecting mid-poll (e.g. a config edit or a dead-handle
    /// replacement) would leave this poller polling a different connection
    /// than the task lives on, making `tasks/get` fail with `-32602` and
    /// silently losing the result.
    ///
    /// Runs until the task reaches a terminal status (successfully handed
    /// off to the injector), the binding is removed out from under it (e.g.
    /// cancelled), or a poll fails (logged and abandoned — the binding is
    /// dropped so a later `cancel_tasks_for_session` does not try to cancel
    /// an already-dead task).
    fn spawn_poller(self: Arc<Self>, task_id: String, mut poll_ms: u64, reg: Arc<McpRegistry>) {
        zeroclaw_spawn::spawn!(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(poll_ms.max(100))).await;

                let Some(server) = self.server_of(&task_id).await else {
                    // Binding was removed (e.g. cancelled) while we slept.
                    return;
                };
                let got = match reg.get_task_on_server(&server, &task_id).await {
                    Ok(g) => g,
                    Err(e) => {
                        ::zeroclaw_log::record!(
                            WARN,
                            ::zeroclaw_log::Event::new(
                                module_path!(),
                                ::zeroclaw_log::Action::Fail
                            )
                            .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                            .with_attrs(
                                ::serde_json::json!({"server": &server, "task_id": &task_id})
                            ),
                            &format!("mcp-task: poll `{task_id}` on `{server}` failed: {e:#}")
                        );
                        self.drop_task(&task_id).await;
                        return;
                    }
                };
                if let Some(p) = got.task.poll_interval_ms {
                    poll_ms = p;
                }
                if got.task.status.is_terminal() {
                    if let Some(binding) = self.drop_task(&task_id).await {
                        self.injector.inject(binding, got).await;
                    }
                    return;
                }
            }
        });
    }

    async fn server_of(&self, task_id: &str) -> Option<String> {
        self.tasks
            .lock()
            .await
            .get(task_id)
            .map(|b| b.server.clone())
    }

    async fn drop_task(&self, task_id: &str) -> Option<TaskBinding> {
        self.tasks.lock().await.remove(task_id)
    }

    /// Cancel every live task bound to `session_key`. Best-effort: a
    /// server-side cancel failure is logged (via the `Result` being
    /// discarded here — the binding is dropped either way) rather than
    /// propagated, since the caller (session teardown) has no useful
    /// recovery action.
    pub async fn cancel_tasks_for_session(self: &Arc<Self>, session_key: &str) {
        let victims: Vec<(String, String, String)> = self
            .tasks
            .lock()
            .await
            .iter()
            .filter(|(_, b)| b.session_key.as_deref() == Some(session_key))
            .map(|(id, b)| (id.clone(), b.agent_alias.clone(), b.server.clone()))
            .collect();
        for (id, alias, server) in victims {
            #[cfg(test)]
            self.cancel_calls.lock().unwrap().push(id.clone());
            if let Some(reg) = self.pool.registry_for(&alias).await {
                let _ = reg.cancel_task_on_server(&server, &id).await;
            }
            self.drop_task(&id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test seam replacing the brief's `InjectRequest`-channel-as-sink
    /// design (Ruling R2): the fake `TaskInjector` renders the
    /// `GetTaskResult` it receives into the same shape `inject_task_completion`
    /// (Task 6) will eventually produce, and pushes it onto a channel the
    /// test drains.
    struct InjectRequest {
        session_key: Option<String>,
        channel: Option<String>,
        reply_target: Option<String>,
        sanitized_text: String,
    }

    struct FakeInjector {
        tx: tokio::sync::mpsc::UnboundedSender<InjectRequest>,
    }

    #[async_trait::async_trait]
    impl TaskInjector for FakeInjector {
        async fn inject(&self, binding: TaskBinding, got: GetTaskResult) {
            let sanitized_text = got
                .result
                .as_ref()
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();
            let _ = self.tx.send(InjectRequest {
                session_key: binding.session_key,
                channel: binding.channel,
                reply_target: binding.reply_target,
                sanitized_text,
            });
        }
    }

    /// A fake scope registry advertising `kutsu__place_call`: `create_task`
    /// returns a `working` task envelope, the first `get_task` poll returns
    /// `working` again, and the second returns `completed` with result text
    /// `"done"` — exercising the poller's re-poll-until-terminal loop.
    async fn fake_scope_registry_two_step() -> Arc<McpRegistry> {
        Arc::new(McpRegistry::for_test_task_two_step("kutsu", "place_call", 10, "done").await)
    }

    #[tokio::test]
    async fn task_polls_to_completion_and_injects() {
        let (sink_tx, mut sink_rx) = tokio::sync::mpsc::unbounded_channel::<InjectRequest>();
        let injector: Arc<dyn TaskInjector> = Arc::new(FakeInjector { tx: sink_tx });
        let sup =
            McpTaskSupervisor::new_for_test("main", fake_scope_registry_two_step().await, injector);

        let disp = sup
            .create_task(
                "main",
                "kutsu",
                "place_call",
                serde_json::json!({}),
                Some("sess-1".into()),
                Some("telegram".into()),
                Some("chat-1".into()),
            )
            .await
            .unwrap();
        assert!(matches!(disp, TaskDispatch::Pending { .. }));

        let injected = tokio::time::timeout(std::time::Duration::from_secs(2), sink_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(injected.session_key.as_deref(), Some("sess-1"));
        // The origin channel/reply-target captured at `create_task` time
        // (Task 8b) must survive through to the binding the poller hands
        // the injector once the task completes.
        assert_eq!(injected.channel.as_deref(), Some("telegram"));
        assert_eq!(injected.reply_target.as_deref(), Some("chat-1"));
        assert!(injected.sanitized_text.contains("done"));

        // The completed task's binding is removed once injected: cancelling
        // the (now-nonexistent) session task is a safe no-op, not an error.
        sup.cancel_tasks_for_session("sess-1").await;
    }

    #[tokio::test]
    async fn admission_cap_rejects_inline_when_scope_is_full() {
        let (sink_tx, _sink_rx) = tokio::sync::mpsc::unbounded_channel::<InjectRequest>();
        let injector: Arc<dyn TaskInjector> = Arc::new(FakeInjector { tx: sink_tx });
        let sup =
            McpTaskSupervisor::new_for_test("main", fake_scope_registry_two_step().await, injector);

        // With no `[mcp_bundles]` configured for "main" in the default test
        // `Config`, `create_task`'s cap lookup falls back to 32 (see
        // `unwrap_or(32)`). Pre-seed that many synthetic live bindings so
        // the next `create_task` call observes `live >= cap` and is
        // rejected inline instead of creating a real task.
        {
            let mut tasks = sup.tasks.lock().await;
            for i in 0..32 {
                tasks.insert(
                    format!("filler-{i}"),
                    TaskBinding {
                        session_key: None,
                        agent_alias: "main".to_string(),
                        server: "kutsu".to_string(),
                        poll_interval_ms: 1000,
                        channel: None,
                        reply_target: None,
                    },
                );
            }
        }

        let disp = sup
            .create_task(
                "main",
                "kutsu",
                "place_call",
                serde_json::json!({}),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        match disp {
            TaskDispatch::Inline(msg) => assert!(msg.contains("Task rejected")),
            TaskDispatch::Pending { .. } => panic!("expected admission cap to reject"),
        }
    }

    /// Task 9: a session interruption (`/stop` or interrupt-on-new-message)
    /// must reach any MCP task still bound to that session — cancelling it
    /// server-side (best-effort) and dropping it from the live-task table so
    /// a later injection attempt has nothing to deliver into.
    #[tokio::test]
    async fn cancel_removes_session_tasks() {
        let sup = McpTaskSupervisor::new_for_test_with_live_task("sess-9", "kutsu", "t9");
        sup.cancel_tasks_for_session("sess-9").await;
        assert!(sup.is_empty().await);
        assert!(sup.cancel_calls().contains(&"t9".to_string()));
    }

    /// Regression: a server that answers a call INLINE (no task envelope) with
    /// a base64 image must have that payload materialized to the agent
    /// workspace and replaced with an `[IMAGE:]` marker — never dumped raw into
    /// the model's context. This is the sync counterpart to the async
    /// `render_task_result` intake; before the fix the `TaskCall::Inline` arm
    /// serialized the raw result and blew the context window.
    #[tokio::test]
    async fn create_task_inline_materializes_base64_image() {
        use base64::Engine;

        let ws = tempfile::tempdir().unwrap();
        let raw = b"inline-raw-image-bytes-must-not-leak";
        let b64 = base64::engine::general_purpose::STANDARD.encode(raw);
        let result = serde_json::json!({
            "content": [{"type": "image", "data": b64.clone(), "mimeType": "image/png"}],
            "isError": false
        });
        let reg = Arc::new(McpRegistry::for_test_inline_returning("glossa", "read", result).await);

        struct NoopInjector;
        #[async_trait::async_trait]
        impl TaskInjector for NoopInjector {
            async fn inject(&self, _binding: TaskBinding, _got: GetTaskResult) {}
        }

        // `config_path`'s parent is the install root; `agent_workspace_dir`
        // resolves under it, so point it at the tempdir to capture the write.
        let mut config = Config::default();
        config.config_path = ws.path().join("config.toml");
        // The materializer opens (does not create) the workspace dir; in
        // production it always exists, so create it here.
        std::fs::create_dir_all(config.agent_workspace_dir("main")).unwrap();

        let mut scopes = HashMap::new();
        scopes.insert("main".to_string(), reg);
        let sup = Arc::new(McpTaskSupervisor {
            config,
            scopes: Mutex::new(scopes),
            tasks: Mutex::new(HashMap::new()),
            injector: Arc::new(NoopInjector),
            test_pending: None,
            last_session_key: std::sync::Mutex::new(None),
            last_origin_route: std::sync::Mutex::new(None),
            cancel_calls: std::sync::Mutex::new(Vec::new()),
        });

        let disp = sup
            .create_task(
                "main",
                "glossa",
                "read",
                serde_json::json!({}),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        match disp {
            TaskDispatch::Inline(text) => {
                assert!(
                    !text.contains(&b64),
                    "raw base64 leaked into inline result ({} chars)",
                    text.len()
                );
                assert!(text.contains("[IMAGE:"), "expected IMAGE marker: {text}");
            }
            TaskDispatch::Pending { .. } => panic!("expected inline result, got pending"),
        }
        assert!(
            ws.path()
                .join("agents")
                .join("main")
                .join("workspace")
                .join("uploads")
                .exists(),
            "image must be materialized under the agent workspace"
        );
    }
}
