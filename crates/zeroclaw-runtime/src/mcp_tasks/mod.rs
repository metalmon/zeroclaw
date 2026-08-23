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

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use zeroclaw_config::schema::Config;
use zeroclaw_tools::mcp_client::TaskCall;
use zeroclaw_tools::mcp_protocol::{GetTaskResult, MODEL_IMMEDIATE_RESPONSE_KEY};

use crate::tools::McpRegistry;

/// Outcome of [`McpTaskSupervisor::create_task`].
pub enum TaskDispatch {
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
}

/// Delivers a completed (or failed/cancelled) task's result back into its
/// owning conversation. Implemented by `mcp_tasks::inject`'s production
/// injector (Task 6); the poller depends only on this trait so the two
/// halves of the feature can be built independently without a circular
/// module dependency.
#[async_trait::async_trait]
pub trait TaskInjector: Send + Sync {
    async fn inject(&self, binding: TaskBinding, got: GetTaskResult);
}

/// Owns per-agent-scope task-advertising MCP connections, admission control,
/// and the background pollers for every in-flight task.
pub struct McpTaskSupervisor {
    config: Config,
    /// One task-advertising registry per agent scope, built lazily.
    scopes: Mutex<HashMap<String, Arc<McpRegistry>>>,
    tasks: Mutex<HashMap<String, TaskBinding>>,
    injector: Arc<dyn TaskInjector>,
}

impl McpTaskSupervisor {
    pub fn new(config: Config, injector: Arc<dyn TaskInjector>) -> Arc<Self> {
        Arc::new(Self {
            config,
            scopes: Mutex::new(HashMap::new()),
            tasks: Mutex::new(HashMap::new()),
            injector,
        })
    }

    /// Test-only: build a supervisor whose `alias` scope is pre-populated
    /// with `registry`, so `scope_registry` never has to connect a real MCP
    /// server. Lets the poller logic (admission, polling, terminal-state
    /// injection) be exercised end to end against a fake registry.
    #[cfg(test)]
    fn new_for_test(
        alias: &str,
        registry: Arc<McpRegistry>,
        injector: Arc<dyn TaskInjector>,
    ) -> Arc<Self> {
        let mut scopes = HashMap::new();
        scopes.insert(alias.to_string(), registry);
        Arc::new(Self {
            config: Config::default(),
            scopes: Mutex::new(scopes),
            tasks: Mutex::new(HashMap::new()),
            injector,
        })
    }

    /// Build (or reuse) this scope's task-advertising registry. Only
    /// servers with `tasks_enabled_effective()` are connected here, each
    /// advertising the tasks extension during its handshake.
    async fn scope_registry(self: &Arc<Self>, alias: &str) -> anyhow::Result<Arc<McpRegistry>> {
        if let Some(r) = self.scopes.lock().await.get(alias) {
            return Ok(Arc::clone(r));
        }
        let servers: Vec<_> = self
            .config
            .mcp_servers_for_agent(alias)
            .into_iter()
            .filter(|s| s.tasks_enabled_effective())
            .collect();
        let reg = Arc::new(McpRegistry::connect_all_advertising_tasks(&servers).await?);
        self.scopes
            .lock()
            .await
            .insert(alias.to_string(), Arc::clone(&reg));
        Ok(reg)
    }

    /// Create a task for `alias` on `server`/`tool`. Subject to a
    /// per-scope, per-server admission cap
    /// (`McpServerConfig::max_concurrent_tasks`); a call over the cap is
    /// rejected inline rather than queued.
    pub async fn create_task(
        self: &Arc<Self>,
        alias: &str,
        server: &str,
        tool: &str,
        args: serde_json::Value,
        session_key: Option<String>,
    ) -> anyhow::Result<TaskDispatch> {
        // Admission cap per scope.
        let cap = self
            .config
            .mcp_servers_for_agent(alias)
            .iter()
            .find(|s| s.name == server)
            .map(|s| s.max_concurrent_tasks())
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

        let reg = self.scope_registry(alias).await?;
        let prefixed = format!("{server}__{tool}");
        match reg.create_task(&prefixed, args).await? {
            TaskCall::Inline(v) => Ok(TaskDispatch::Inline(serde_json::to_string_pretty(&v)?)),
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
                    },
                );
                self.clone()
                    .spawn_poller(alias.to_string(), ct.task.task_id.clone(), poll);
                Ok(TaskDispatch::Pending { immediate })
            }
        }
    }

    /// Background poll loop for one task. Runs until the task reaches a
    /// terminal status (successfully handed off to the injector), the
    /// binding is removed out from under it (e.g. cancelled), or a poll
    /// fails (logged and abandoned — the binding is dropped so a later
    /// `cancel_tasks_for_session` does not try to cancel an already-dead
    /// task).
    fn spawn_poller(self: Arc<Self>, alias: String, task_id: String, mut poll_ms: u64) {
        zeroclaw_spawn::spawn!(async move {
            let reg = match self.scope_registry(&alias).await {
                Ok(r) => r,
                Err(e) => {
                    ::zeroclaw_log::record!(
                        WARN,
                        ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                            .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                            .with_attrs(
                                ::serde_json::json!({"agent_alias": &alias, "task_id": &task_id})
                            ),
                        &format!("mcp-task: scope registry for `{alias}` unavailable: {e:#}")
                    );
                    self.drop_task(&task_id).await;
                    return;
                }
            };
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
            if let Ok(reg) = self.scope_registry(&alias).await {
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
            )
            .await
            .unwrap();
        assert!(matches!(disp, TaskDispatch::Pending { .. }));

        let injected = tokio::time::timeout(std::time::Duration::from_secs(2), sink_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(injected.session_key.as_deref(), Some("sess-1"));
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
                    },
                );
            }
        }

        let disp = sup
            .create_task("main", "kutsu", "place_call", serde_json::json!({}), None)
            .await
            .unwrap();
        match disp {
            TaskDispatch::Inline(msg) => assert!(msg.contains("Task rejected")),
            TaskDispatch::Pending { .. } => panic!("expected admission cap to reject"),
        }
    }
}
