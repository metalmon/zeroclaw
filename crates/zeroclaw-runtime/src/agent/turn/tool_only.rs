//! Gated tool-only turn entry point.
//!
//! Runs a single pre-formed tool call through ZeroClaw's existing
//! authorization gate (`prepare_tool_calls`) and the executor, with no
//! LLM/provider call anywhere in the path. This lets a non-model-driven
//! caller (e.g. a canvas action channel) submit a tool call that is gated
//! identically to a model-emitted one: allowed-tools membership, approval,
//! and the forced `approved=false` reset all still apply.

use super::call_prep::prepare_tool_calls;
use super::context::TurnCtx;
use super::events::resolve_tool_call_id;
use crate::agent::tool_execution::{
    ToolDispatchContext, ToolExecutionOutcome, execute_tools_sequential, resolved_tool_provenance,
};
use std::collections::HashSet;
use zeroclaw_api::agent::TurnEvent;
use zeroclaw_tool_call_parser::ParsedToolCall;

/// Outcome of a gated tool-only turn.
pub struct ToolOnlyOutcome {
    /// `true` when nothing executed: the approval gate denied the call, a
    /// `before_tool_call` hook cancelled it, it was deduplicated away, or the
    /// named tool does not resolve in the registry.
    pub gated_out: bool,
    /// The executed tool's outcome. `Some` only when the call actually ran.
    pub outcome: Option<ToolExecutionOutcome>,
}

/// Run a single pre-formed tool call through the same gate a model-emitted
/// tool call goes through (`prepare_tool_calls`), then execute it — with no
/// provider/LLM call anywhere in this function.
///
/// `call.tool_call_id` should already carry the caller's own correlation id
/// (e.g. `"canvas:<callId>"`); it is threaded through unchanged to the
/// emitted `TurnEvent::ToolCall`/`TurnEvent::ToolResult` pair when the call
/// actually executes.
///
/// `prepare_tool_calls` gates hooks/approval/dedup but does not itself check
/// whether the named tool resolves in the registry — that check normally
/// lives inside the executor and would otherwise surface as a "ran but
/// failed" outcome. This function checks resolvability itself (after the
/// gate, so it never skips approval) and reports an unresolvable tool as
/// `gated_out` instead of a failed execution.
pub async fn run_tool_only_turn(
    ctx: &TurnCtx<'_>,
    tools_registry: &[Box<dyn crate::tools::Tool>],
    activated_tools: Option<&std::sync::Arc<std::sync::Mutex<crate::tools::ActivatedToolSet>>>,
    call: ParsedToolCall,
) -> anyhow::Result<ToolOnlyOutcome> {
    let mut seen_tool_signatures = HashSet::new();
    let mut prompt_approval_tool_signatures = HashSet::new();

    let prepared = prepare_tool_calls(
        ctx,
        tools_registry,
        activated_tools,
        std::slice::from_ref(&call),
        &mut seen_tool_signatures,
        &mut prompt_approval_tool_signatures,
        0,
        false,
    )
    .await?;

    let Some(executable_call) = prepared.executable_calls.first() else {
        // Denied by the approval gate, cancelled by a hook, or deduplicated
        // away. `prepare_tool_calls` already emitted whatever outcome event
        // that path calls for (e.g. a synthesized denial `ToolResult`).
        return Ok(ToolOnlyOutcome {
            gated_out: true,
            outcome: None,
        });
    };

    if resolved_tool_provenance(tools_registry, activated_tools, &executable_call.name).is_none() {
        // A caller (e.g. a canvas action) can name a tool that does not resolve
        // in this agent's registry — a virtual/deferred tool, or a typo. Surface
        // it as a failed `ToolResult` correlated by the caller's own id rather
        // than returning silently: an app-initiated `tools/call` that gets no
        // correlated result hangs on "no result" and retries indefinitely.
        if let Some(tx) = ctx.event_tx {
            let id = resolve_tool_call_id(&call);
            let _ = tx
                .send(TurnEvent::ToolCall {
                    id: id.clone(),
                    name: call.name.clone(),
                    args: call.arguments.clone(),
                })
                .await;
            let _ = tx
                .send(TurnEvent::ToolResult {
                    id,
                    name: call.name.clone(),
                    output: format!("Tool '{}' is not available.", call.name),
                    artifact: None,
                    ui_resource: None,
                })
                .await;
        }
        return Ok(ToolOnlyOutcome {
            gated_out: true,
            outcome: None,
        });
    }

    let dispatch = ToolDispatchContext {
        tools_registry,
        activated_tools,
        excluded_tools: &[],
        model_switch_callback: None,
    };
    let meta = ctx.meta();
    let mut outcomes = execute_tools_sequential(
        &prepared.executable_calls,
        dispatch,
        &meta,
        ctx.observer,
        ctx.cancellation_token,
        None,
        ctx.event_tx,
    )
    .await?;

    Ok(ToolOnlyOutcome {
        gated_out: false,
        outcome: outcomes.pop().flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::{ToolOnlyOutcome, TurnCtx, run_tool_only_turn};
    use crate::approval::ApprovalManager;
    use crate::observability::noop::NoopObserver;
    use crate::security::AutonomyLevel;
    use crate::tools::scoped::ScopedToolRegistry;
    use crate::tools::{Tool, ToolResult};
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;
    use zeroclaw_api::agent::TurnEvent;
    use zeroclaw_api::attribution::{Attributable, Role};
    use zeroclaw_config::schema::{PacingConfig, RiskProfileConfig, StreamReasoningMode};
    use zeroclaw_tool_call_parser::ParsedToolCall;

    /// Minimal tool that counts invocations, so tests can assert whether the
    /// gate actually let execution reach it.
    struct CountingEchoTool {
        name: String,
        invocations: Arc<AtomicUsize>,
    }

    impl Attributable for CountingEchoTool {
        fn role(&self) -> Role {
            Role::System
        }

        fn alias(&self) -> &str {
            "test-canvas-tool"
        }
    }

    #[async_trait]
    impl Tool for CountingEchoTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Counts invocations for tool-only-turn gating tests"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}, "required": []})
        }

        async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
            self.invocations.fetch_add(1, Ordering::SeqCst);
            Ok(ToolResult {
                success: true,
                output: "echoed".into(),
                error: None,
            })
        }
    }

    fn test_pacing() -> PacingConfig {
        PacingConfig::default()
    }

    fn test_ctx<'a>(
        observer: &'a NoopObserver,
        pacing: &'a PacingConfig,
        approval: Option<&'a ApprovalManager>,
        event_tx: Option<&'a mpsc::Sender<TurnEvent>>,
    ) -> TurnCtx<'a> {
        TurnCtx {
            observer,
            provider_name: "test",
            model: "test-model",
            temperature: None,
            approval,
            channel_name: "test",
            channel_reply_target: None,
            cancellation_token: None,
            on_delta: None,
            event_tx,
            hooks: None,
            dedup_exempt_tools: &[],
            pacing,
            strict_tool_parsing: false,
            channel: None,
            draft_reasoning: StreamReasoningMode::Status,
            turn_id: "test-turn",
            agent_alias: None,
            parent_agent_alias: None,
        }
    }

    #[tokio::test]
    async fn allowed_auto_approved_tool_runs_and_emits_tool_result() {
        let observer = NoopObserver;
        let pacing = test_pacing();
        let approval = ApprovalManager::from_risk_profile(&RiskProfileConfig {
            level: AutonomyLevel::Full,
            ..Default::default()
        });
        let (tx, mut rx) = mpsc::channel::<TurnEvent>(8);
        let ctx = test_ctx(&observer, &pacing, Some(&approval), Some(&tx));

        let invocations = Arc::new(AtomicUsize::new(0));
        let registry = ScopedToolRegistry::from_raw_for_test(vec![Box::new(CountingEchoTool {
            name: "canvas_echo".to_string(),
            invocations: Arc::clone(&invocations),
        })]);

        let call = ParsedToolCall {
            name: "canvas_echo".to_string(),
            arguments: serde_json::json!({}),
            tool_call_id: Some("canvas:n1:1".to_string()),
        };

        let ToolOnlyOutcome { gated_out, outcome } =
            run_tool_only_turn(&ctx, &registry, None, call)
                .await
                .expect("tool-only turn should not error");

        assert!(
            !gated_out,
            "an allowed, auto-approved call must not be gated out"
        );
        let outcome = outcome.expect("the tool actually ran");
        assert!(outcome.success);
        assert_eq!(invocations.load(Ordering::SeqCst), 1);

        drop(tx);
        let mut saw_result = false;
        while let Some(event) = rx.recv().await {
            if let TurnEvent::ToolResult { id, .. } = event {
                assert_eq!(id, "canvas:n1:1", "ToolResult must carry the caller's id");
                saw_result = true;
            }
        }
        assert!(
            saw_result,
            "expected a ToolResult event for the executed call"
        );
    }

    #[tokio::test]
    async fn approval_required_and_denied_gates_out_without_running() {
        let observer = NoopObserver;
        let pacing = test_pacing();
        // Supervised + non-interactive + no channel: the approval gate falls
        // back to auto-deny for any tool it prompts for.
        let approval = ApprovalManager::for_non_interactive(&RiskProfileConfig::default());
        let (tx, mut rx) = mpsc::channel::<TurnEvent>(8);
        let ctx = test_ctx(&observer, &pacing, Some(&approval), Some(&tx));

        let invocations = Arc::new(AtomicUsize::new(0));
        let registry = ScopedToolRegistry::from_raw_for_test(vec![Box::new(CountingEchoTool {
            name: "canvas_dangerous".to_string(),
            invocations: Arc::clone(&invocations),
        })]);

        let call = ParsedToolCall {
            name: "canvas_dangerous".to_string(),
            arguments: serde_json::json!({}),
            tool_call_id: Some("canvas:n2:1".to_string()),
        };

        let ToolOnlyOutcome { gated_out, outcome } =
            run_tool_only_turn(&ctx, &registry, None, call)
                .await
                .expect("tool-only turn should not error");

        assert!(gated_out, "a denied call must be reported as gated out");
        assert!(
            outcome.is_none(),
            "a denied call must not produce an execution outcome"
        );
        assert_eq!(
            invocations.load(Ordering::SeqCst),
            0,
            "the tool's execute() must never run when the approval gate denies the call"
        );

        // `prepare_tool_calls` synthesizes a denial ToolCall/ToolResult pair
        // on the deny path (parity with a model-emitted denied call) — that
        // is expected and is NOT the real tool running. Any ToolResult seen
        // here must carry the denial, never a successful "echoed" output.
        drop(tx);
        while let Some(event) = rx.recv().await {
            if let TurnEvent::ToolResult { output, .. } = event {
                assert_ne!(
                    output, "echoed",
                    "a denied call's ToolResult must not be the tool's own output"
                );
            }
        }
    }

    #[tokio::test]
    async fn tool_absent_from_registry_gates_out() {
        let observer = NoopObserver;
        let pacing = test_pacing();
        let approval = ApprovalManager::from_risk_profile(&RiskProfileConfig {
            level: AutonomyLevel::Full,
            ..Default::default()
        });
        let (tx, mut rx) = mpsc::channel::<TurnEvent>(8);
        let ctx = test_ctx(&observer, &pacing, Some(&approval), Some(&tx));

        let registry = ScopedToolRegistry::from_raw_for_test(vec![]);

        let call = ParsedToolCall {
            name: "does_not_exist".to_string(),
            arguments: serde_json::json!({}),
            tool_call_id: Some("canvas:n3:1".to_string()),
        };

        let ToolOnlyOutcome { gated_out, outcome } =
            run_tool_only_turn(&ctx, &registry, None, call)
                .await
                .expect("tool-only turn should not error");

        assert!(
            gated_out,
            "an unresolvable tool must be reported as gated out"
        );
        assert!(outcome.is_none());

        // The unresolvable tool must still surface an observable, correlated
        // failure carrying the caller's canvas id — never a silent return, which
        // leaves an app-initiated `tools/call` hung on "no result" and retrying.
        drop(tx);
        let mut saw_failure = false;
        while let Some(event) = rx.recv().await {
            if let TurnEvent::ToolResult { id, output, .. } = event {
                assert_eq!(id, "canvas:n3:1", "ToolResult must carry the caller's id");
                assert!(
                    output.contains("not available"),
                    "unresolvable tool result must explain the failure: {output}"
                );
                saw_failure = true;
            }
        }
        assert!(
            saw_failure,
            "an unresolvable canvas tool must emit a failed ToolResult, not stay silent"
        );
    }
}
