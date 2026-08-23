//! Sanitizes a completed (or failed/cancelled) MCP task's result and injects
//! it back into the conversation that started it, via a reactive
//! [`crate::agent::run`] turn tagged [`TurnOrigin::McpTask`].
//!
//! This is the production [`TaskInjector`] the poller in
//! `mcp_tasks::mod` depends on (see that module's doc comment for why the
//! dependency runs poller -> trait -> this module, and never the reverse).

use std::path::PathBuf;

use zeroclaw_api::ingress::TurnOrigin;
use zeroclaw_config::schema::Config;
use zeroclaw_tools::mcp_protocol::{GetTaskResult, TaskStatus};

use super::{TaskBinding, TaskInjector};

/// Scrub server-origin text and wrap it as untrusted-external, mirroring
/// `zeroclaw_tools::mcp_context::wrap_resource_contents`: everything a
/// remote MCP server sent back is server-controlled and must never be
/// trusted as instructions, so it is scrubbed with the same
/// `sanitize_api_error` used for tool-result text and wrapped in a
/// `trust="untrusted-external"` provenance tag.
pub fn render_task_result(server: &str, task_id: &str, got: &GetTaskResult) -> String {
    let body = match got.task.status {
        TaskStatus::Completed => got
            .result
            .as_ref()
            .map(|r| serde_json::to_string_pretty(r).unwrap_or_default())
            .unwrap_or_else(|| "(completed, no result payload)".into()),
        TaskStatus::Failed => got
            .error
            .as_ref()
            .map(|e| serde_json::to_string_pretty(e).unwrap_or_default())
            .or_else(|| got.task.status_message.clone())
            .unwrap_or_else(|| "(failed)".into()),
        TaskStatus::Cancelled => "(cancelled)".into(),
        other => format!("(status: {other:?})"),
    };
    let scrubbed = zeroclaw_providers::sanitize_api_error(&body);
    format!(
        "<mcp-task server=\"{server}\" taskId=\"{task_id}\" status=\"{status}\" trust=\"untrusted-external\">\n{scrubbed}\n</mcp-task>",
        status = serde_json::to_string(&got.task.status)
            .unwrap_or_default()
            .trim_matches('"'),
    )
}

/// Production [`TaskInjector`]: renders the task result and runs a reactive
/// turn against the session that created it.
///
/// **Delivery scope (v1 / Ruling R8):** `TaskBinding` carries a
/// `session_key` but no channel/reply-target — the poller only ever saw the
/// tool call, not which channel (Telegram, CLI, ACP, ...) originated it. So
/// this injector does the part it can do soundly: append the rendered
/// result as the user turn of a reactive `agent::run` call, so the model
/// reacts to it and the exchange lands in that session's memory/log via the
/// normal `agent::run` machinery. Pushing the reactive reply out to the
/// origin channel's transport (e.g. sending a new Telegram message) needs a
/// reply-target that does not exist on `TaskBinding` yet; wiring that is
/// deferred to Task 8, which is expected to extend `TaskBinding` (or thread
/// a channel adapter through `McpTaskSupervisor`) once the shape of that
/// delivery is decided.
pub struct RuntimeInjector {
    pub config: Config,
}

#[async_trait::async_trait]
impl TaskInjector for RuntimeInjector {
    async fn inject(&self, binding: TaskBinding, got: GetTaskResult) {
        let Some(session_key) = binding.session_key.clone() else {
            ::zeroclaw_log::record!(
                INFO,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Skip)
                    .with_attrs(::serde_json::json!({
                        "task_id": &got.task.task_id,
                        "server": &binding.server,
                    })),
                "mcp-task: completed with no bound session; dropping result"
            );
            return;
        };
        let rendered = render_task_result(&binding.server, &got.task.task_id, &got);

        // `session_state_file` is a literal caller-chosen label, not a
        // resolved absolute path — mirrors `sop::executor::drive_headless_run`
        // (`PathBuf::from(format!("sop-{run_id}-step-{}", step.number))`),
        // the only existing precedent for turning an arbitrary string key
        // into `agent::run`'s `session_state_file` for a single-shot
        // (`Some(message)`) call. In that call shape the value is never read
        // back as a JSONL transcript (that only happens in the interactive
        // CLI REPL branch) — it is sanitized into `memory_session_id`
        // (`agent::loop_::run`, `sanitize_session_key(&format!("cli:{raw}"))`)
        // and used to scope this turn's memory auto-save/recall. That keeps
        // the exchange associated with `session_key` in memory even though
        // it does not (yet) resolve to the same session-id scheme
        // `agent::loop_::process_message` uses for live channel turns.
        let session_state_file = Some(PathBuf::from(&session_key));

        let result = crate::agent::run(
            self.config.clone(),
            &binding.agent_alias,
            Some(rendered),
            None,
            None,
            None,
            Vec::new(),
            /* interactive */ false,
            session_state_file,
            None,
            TurnOrigin::McpTask,
            crate::agent::loop_::AgentRunOverrides::default(),
        )
        .await;

        if let Err(e) = result {
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Fail)
                    .with_outcome(::zeroclaw_log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({
                        "task_id": &got.task.task_id,
                        "server": &binding.server,
                        "session_key": &session_key,
                    })),
                &format!("mcp-task: reactive injection turn failed: {e:#}")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_completed_result_untrusted_wrapped() {
        let got: GetTaskResult = serde_json::from_value(serde_json::json!({
            "resultType":"complete","taskId":"t1","status":"completed",
            "createdAt":"t","lastUpdatedAt":"t",
            "result":{"content":[{"type":"text","text":"call ended: 42s"}],"isError":false}
        }))
        .unwrap();
        let s = render_task_result("kutsu", "t1", &got);
        assert!(s.contains("trust=\"untrusted-external\""));
        assert!(s.contains("call ended: 42s"));
        assert!(s.contains("t1"));
    }

    #[test]
    fn renders_failed_result_as_error_context() {
        let got: GetTaskResult = serde_json::from_value(serde_json::json!({
            "resultType":"complete","taskId":"t2","status":"failed",
            "createdAt":"t","lastUpdatedAt":"t","statusMessage":"line busy"
        }))
        .unwrap();
        let s = render_task_result("kutsu", "t2", &got);
        assert!(s.contains("failed"));
        assert!(s.contains("line busy"));
    }
}
