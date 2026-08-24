//! [`McpTaskToolWrapper`]: the [`zeroclaw_api::tool::Tool`] installed for an
//! MCP tool whose server advertises the `io.modelcontextprotocol/tasks`
//! extension (`tasks_enabled_effective()`), in place of the plain
//! `zeroclaw_tools::mcp_tool::McpToolWrapper`.
//!
//! Shape mirrors `McpToolWrapper` (prefixed name, `Arc`-shared schema,
//! `approved`-field stripping before the call goes out) but `execute`
//! dispatches through [`super::McpTaskSupervisor::create_task`] instead of
//! calling the MCP server directly, so a long-running tool call polls in
//! the background rather than blocking the agent turn; the eventual result
//! is delivered later, out of band, via `mcp_tasks::inject`.

use std::sync::Arc;

use async_trait::async_trait;
use zeroclaw_api::tool::{Tool, ToolOutput, ToolResult, ToolSpec};

use super::{McpTaskSupervisor, TaskDispatch};

/// A zeroclaw [`Tool`] backed by a task-enabled MCP server tool.
pub(crate) struct McpTaskToolWrapper {
    /// Prefixed name: `<server_name>__<tool_name>`.
    pub(crate) prefixed_name: String,
    pub(crate) description: String,
    /// JSON schema for the tool's input parameters. `Arc`-shared for the
    /// same reason as `McpToolWrapper`: per-iteration spec assembly hands
    /// out reference counts instead of deep-cloning the tree.
    pub(crate) input_schema: Arc<serde_json::Value>,
    /// Unprefixed MCP server name (the part of `prefixed_name` before `__`).
    pub(crate) server: String,
    /// Unprefixed MCP tool name (the part of `prefixed_name` after `__`).
    pub(crate) tool: String,
    pub(crate) agent_alias: String,
    pub(crate) supervisor: Arc<McpTaskSupervisor>,
}

#[async_trait]
impl Tool for McpTaskToolWrapper {
    fn name(&self) -> &str {
        &self.prefixed_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        (*self.input_schema).clone()
    }

    /// Override the default: hand out the stored schema by `Arc::clone`
    /// instead of deep-cloning it, matching `McpToolWrapper::spec`.
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.prefixed_name.clone(),
            description: self.description.clone(),
            parameters: Arc::clone(&self.input_schema),
            output: None,
            param_domains: std::collections::BTreeMap::new(),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let args = match args {
            serde_json::Value::Object(mut map) => {
                map.remove("approved");
                serde_json::Value::Object(map)
            }
            other => other,
        };
        // Same idiom as `crate::tools::shell::get_session_id`: the agent
        // loop scopes this task-local for the duration of a turn so tool
        // calls can bind their side effects (here, a background task) to
        // the session that made them.
        let session_key = zeroclaw_api::TOOL_LOOP_SESSION_KEY
            .try_with(Clone::clone)
            .ok()
            .flatten()
            .filter(|key| !key.is_empty());
        match self
            .supervisor
            .create_task(
                &self.agent_alias,
                &self.server,
                &self.tool,
                args,
                session_key,
            )
            .await
        {
            Ok(TaskDispatch::Pending { immediate }) => Ok(ToolResult {
                success: true,
                output: immediate.into(),
                error: None,
            }),
            Ok(TaskDispatch::Inline(text)) => Ok(ToolResult {
                success: true,
                output: text.into(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: ToolOutput::default(),
                error: Some(e.to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_tasks::McpTaskSupervisor;

    #[tokio::test]
    async fn wrapper_binds_session_and_returns_placeholder() {
        let sup = McpTaskSupervisor::new_for_test_pending("queued-msg");
        let w = McpTaskToolWrapper {
            prefixed_name: "kutsu__place_call".into(),
            description: "place a call".into(),
            input_schema: std::sync::Arc::new(serde_json::json!({})),
            server: "kutsu".into(),
            tool: "place_call".into(),
            agent_alias: "main".into(),
            supervisor: sup.clone(),
        };
        let out = zeroclaw_api::TOOL_LOOP_SESSION_KEY
            .scope(Some("sess-7".into()), async {
                w.execute(serde_json::json!({})).await
            })
            .await
            .unwrap();
        assert!(out.success);
        // `ToolOutput: Deref<Target = str>`, so `contains` is available directly.
        assert!(out.output.contains("queued-msg"));
        assert_eq!(sup.last_session_key(), Some("sess-7".into()));
    }
}
