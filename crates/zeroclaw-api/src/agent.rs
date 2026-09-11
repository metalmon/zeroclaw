use crate::plan::PlanEntry;

/// Structured metadata for a tool that produced a file artifact (e.g.
/// `deliver_file`). Carried on [`TurnEvent::ToolResult`] so a channel attaches
/// the file from typed fields instead of parsing a text trailer out of the
/// free-form `output` string. Trailer parsing let a crafted filename forge the
/// delivered path (arbitrary-file-read / confused-deputy class).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolArtifact {
    /// Absolute path of the delivered file on the agent host.
    pub path: String,
    /// Stable citation URI the client can reference (e.g. `attachment://…`).
    pub uri: String,
    /// Original filename.
    pub filename: String,
    /// Human-readable chat label; defaults to the filename.
    pub title: String,
    /// MIME type.
    pub mime: String,
    /// Size in bytes.
    pub size: u64,
}

impl ToolArtifact {
    /// Build from a tool's structured `output_data` when it declares a delivered
    /// file (`delivered: true` with a non-empty `path`). Returns `None` for any
    /// other structured output, keeping this a channel-neutral convention rather
    /// than a hook tied to one tool name.
    pub fn from_delivered_data(data: &serde_json::Value) -> Option<Self> {
        if data.get("delivered").and_then(serde_json::Value::as_bool) != Some(true) {
            return None;
        }
        let field = |key: &str| {
            data.get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let path = field("path");
        if path.is_empty() {
            return None;
        }
        Some(Self {
            uri: field("uri"),
            filename: field("filename"),
            title: field("title"),
            mime: field("mimeType"),
            size: data
                .get("bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            path,
        })
    }
}

/// Structured metadata for a UI resource artifact (e.g. a canvas HTML page
/// delivered via `ui://` protocol). Parsed from structured `output_data` with
/// `ui_resource: true` marker, making it channel-neutral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiResource {
    /// URI of the resource (e.g. `ui://pnl/dashboard`).
    pub uri: String,
    /// MIME type (defaults to `text/html` if not provided).
    pub mime: String,
    /// Full HTML/text content of the resource.
    pub text: String,
}

impl UiResource {
    /// Build from structured `output_data` when a tool declares a UI resource
    /// (`ui_resource: true` with non-empty `uri` and `text`). Returns `None` if
    /// `ui_resource != true`, or any of `uri`/`text` empty.
    pub fn from_ui_resource_data(data: &serde_json::Value) -> Option<Self> {
        if !data.get("ui_resource")?.as_bool()? {
            return None;
        }
        let uri = data.get("uri")?.as_str()?.to_string();
        let text = data.get("text")?.as_str()?.to_string();
        if uri.is_empty() || text.is_empty() {
            return None;
        }
        let mime = data
            .get("mimeType")
            .and_then(|v| v.as_str())
            .unwrap_or("text/html")
            .to_string();
        Some(Self { uri, mime, text })
    }
}

#[derive(Debug, Clone)]
pub enum TurnEvent {
    /// A text chunk from the LLM response (may arrive many times).
    Chunk {
        delta: String,
    },
    /// A reasoning/thinking chunk from a thinking model (may arrive many times).
    Thinking {
        delta: String,
    },
    /// The agent is invoking a tool.
    ToolCall {
        /// Stable correlation ID shared with the matching [`TurnEvent::ToolResult`].
        id: String,
        name: String,
        args: serde_json::Value,
    },
    /// A tool has returned a result.
    ToolResult {
        /// Stable correlation ID shared with the originating [`TurnEvent::ToolCall`].
        id: String,
        name: String,
        output: String,
        /// Typed metadata for a file-producing tool (e.g. `deliver_file`), so
        /// channels attach the file structurally instead of parsing `output`.
        /// `None` for ordinary tools.
        artifact: Option<ToolArtifact>,
        /// Typed metadata for a UI resource artifact (e.g. a canvas HTML page
        /// delivered via `ui://` protocol). `None` for ordinary tools.
        ui_resource: Option<UiResource>,
    },
    Plan {
        entries: Vec<PlanEntry>,
    },
    ApprovalRequest {
        /// Correlation ID. The matching response frame must echo it.
        request_id: String,
        tool_name: String,
        /// Human-readable, secret-redacted summary of the tool arguments.
        /// Synthesised by `crate::approval::summarize_args`; never the raw
        /// `args` value.
        arguments_summary: String,
        /// How long the channel will wait before auto-denying.
        timeout_secs: u64,
    },
    /// Older whole turns were dropped to fit either the context token budget or
    /// the configured message limit. Surfaces a user-visible "context was cut
    /// here" marker so trimming is never silent. Emitted whenever a trim occurs.
    HistoryTrimmed {
        dropped_messages: usize,
        kept_turns: usize,
        reason: String,
    },
    /// Per-LLM-call token usage and cost; a turn may emit several, one per
    /// model call. `None` means "unavailable for this call", not zero.
    Usage {
        input_tokens: Option<u64>,
        /// Tokens served from the provider's prompt cache (e.g. Anthropic
        /// `cache_read_input_tokens`, OpenAI `cached_tokens`). These count
        /// toward the context window and must be added to `input_tokens` to
        /// get the true total context size.
        cached_input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cost_usd: Option<f64>,
    },
}

#[cfg(test)]
mod plan_event_tests {
    use super::*;
    use crate::plan::{PlanEntry, PlanPriority, PlanStatus};

    #[test]
    fn plan_turn_event_carries_entries() {
        let ev = TurnEvent::Plan {
            entries: vec![PlanEntry {
                content: "Step one".to_string(),
                status: PlanStatus::Pending,
                priority: PlanPriority::Medium,
                active_form: None,
            }],
        };
        match ev {
            TurnEvent::Plan { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].content, "Step one");
            }
            _ => panic!("expected TurnEvent::Plan"),
        }
    }
}

#[cfg(test)]
mod tool_artifact_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn projects_delivered_data_into_typed_fields() {
        let data = json!({
            "delivered": true,
            "uri": "attachment://deliver/report.pdf",
            "path": "/ws/uploads/ab.pdf",
            "filename": "report.pdf",
            "title": "Quarterly report",
            "mimeType": "application/pdf",
            "bytes": 1234,
        });
        let a = ToolArtifact::from_delivered_data(&data).expect("delivered data yields artifact");
        assert_eq!(a.path, "/ws/uploads/ab.pdf");
        assert_eq!(a.uri, "attachment://deliver/report.pdf");
        assert_eq!(a.filename, "report.pdf");
        assert_eq!(a.title, "Quarterly report");
        assert_eq!(a.mime, "application/pdf");
        assert_eq!(a.size, 1234);
    }

    #[test]
    fn non_delivered_data_is_ignored() {
        // Ordinary structured tool output must not be mistaken for a file artifact.
        assert!(ToolArtifact::from_delivered_data(&json!({"result": 42})).is_none());
        assert!(
            ToolArtifact::from_delivered_data(&json!({"delivered": false, "path": "/x"})).is_none()
        );
    }

    #[test]
    fn delivered_without_path_is_ignored() {
        assert!(ToolArtifact::from_delivered_data(&json!({"delivered": true})).is_none());
        assert!(
            ToolArtifact::from_delivered_data(&json!({"delivered": true, "path": ""})).is_none()
        );
    }
}

#[cfg(test)]
mod ui_resource_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ui_resource_parses_structured_data() {
        let d = json!({
            "ui_resource": true, "uri": "ui://pnl/dashboard",
            "mimeType": "text/html", "text": "<!doctype html><title>x</title>"
        });
        let r = UiResource::from_ui_resource_data(&d).expect("parses");
        assert_eq!(r.uri, "ui://pnl/dashboard");
        assert_eq!(r.mime, "text/html");
        assert!(r.text.starts_with("<!doctype"));
    }

    #[test]
    fn ui_resource_rejects_non_ui_or_empty() {
        assert!(UiResource::from_ui_resource_data(&json!({"result": 1})).is_none());
        assert!(UiResource::from_ui_resource_data(
            &json!({"ui_resource": true, "uri": "", "text": "x"})).is_none());
        assert!(UiResource::from_ui_resource_data(
            &json!({"ui_resource": false, "uri": "ui://a", "text": "x"})).is_none());
    }
}
