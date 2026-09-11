//! Live Canvas (A2UI) tool — push rendered content to a web canvas in real time.

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use zeroclaw_api::tool::{Tool, ToolOutput, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;

/// Maximum content size per canvas frame (256 KB).
pub const MAX_CONTENT_SIZE: usize = 256 * 1024;

/// Maximum number of history frames kept per canvas.
const MAX_HISTORY_FRAMES: usize = 50;

/// Broadcast channel capacity per canvas.
const BROADCAST_CAPACITY: usize = 64;

/// Maximum number of concurrent canvases to prevent memory exhaustion.
const MAX_CANVAS_COUNT: usize = 100;

/// Allowed content types for canvas frames via the REST API.
pub const ALLOWED_CONTENT_TYPES: &[&str] = &["html", "svg", "markdown", "text"];

/// A single canvas frame (one render).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasFrame {
    /// Unique frame identifier.
    pub frame_id: String,
    /// Content type: `html`, `svg`, `markdown`, or `text`.
    pub content_type: String,
    /// The rendered content.
    pub content: String,
    /// ISO-8601 timestamp of when the frame was created.
    pub timestamp: String,
}

/// Per-canvas state: current content + history + broadcast sender.
struct CanvasEntry {
    current: Option<CanvasFrame>,
    history: Vec<CanvasFrame>,
    tx: broadcast::Sender<CanvasFrame>,
}

/// Shared canvas store — holds all active canvases.
/// Thread-safe and cheaply cloneable (wraps `Arc`).
#[derive(Clone)]
pub struct CanvasStore {
    inner: Arc<RwLock<HashMap<String, CanvasEntry>>>,
}

impl Default for CanvasStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Push a new frame to a canvas. Creates the canvas if it does not exist.
    /// Returns `None` if the maximum canvas count has been reached and this is a new canvas.
    pub fn render(
        &self,
        canvas_id: &str,
        content_type: &str,
        content: &str,
    ) -> Option<CanvasFrame> {
        let frame = CanvasFrame {
            frame_id: uuid::Uuid::new_v4().to_string(),
            content_type: content_type.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let mut store = self.inner.write();

        // Enforce canvas count limit for new canvases.
        if !store.contains_key(canvas_id) && store.len() >= MAX_CANVAS_COUNT {
            return None;
        }

        let entry = store
            .entry(canvas_id.to_string())
            .or_insert_with(|| CanvasEntry {
                current: None,
                history: Vec::new(),
                tx: broadcast::channel(BROADCAST_CAPACITY).0,
            });

        entry.current = Some(frame.clone());
        entry.history.push(frame.clone());
        if entry.history.len() > MAX_HISTORY_FRAMES {
            let excess = entry.history.len() - MAX_HISTORY_FRAMES;
            entry.history.drain(..excess);
        }

        // Best-effort broadcast — ignore errors (no receivers is fine).
        let _ = entry.tx.send(frame.clone());

        Some(frame)
    }

    /// Get the current (most recent) frame for a canvas.
    pub fn snapshot(&self, canvas_id: &str) -> Option<CanvasFrame> {
        let store = self.inner.read();
        store.get(canvas_id).and_then(|entry| entry.current.clone())
    }

    /// Get the current (most recent) frame for a canvas.
    ///
    /// Alias of [`CanvasStore::snapshot`], used by isolation checks that read
    /// back whether a render was actually persisted to the shared store.
    pub fn current(&self, canvas_id: &str) -> Option<CanvasFrame> {
        self.snapshot(canvas_id)
    }

    /// Get the frame history for a canvas.
    pub fn history(&self, canvas_id: &str) -> Vec<CanvasFrame> {
        let store = self.inner.read();
        store
            .get(canvas_id)
            .map(|entry| entry.history.clone())
            .unwrap_or_default()
    }

    /// Clear a canvas (removes current content and history).
    pub fn clear(&self, canvas_id: &str) -> bool {
        let mut store = self.inner.write();
        if let Some(entry) = store.get_mut(canvas_id) {
            entry.current = None;
            entry.history.clear();
            // Send an empty frame to signal clear to subscribers.
            let clear_frame = CanvasFrame {
                frame_id: uuid::Uuid::new_v4().to_string(),
                content_type: "clear".to_string(),
                content: String::new(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            let _ = entry.tx.send(clear_frame);
            true
        } else {
            false
        }
    }

    /// Subscribe to real-time updates for a canvas.
    /// Creates the canvas entry if it does not exist (subject to canvas count limit).
    /// Returns `None` if the canvas does not exist and the limit has been reached.
    pub fn subscribe(&self, canvas_id: &str) -> Option<broadcast::Receiver<CanvasFrame>> {
        let mut store = self.inner.write();

        // Enforce canvas count limit for new entries.
        if !store.contains_key(canvas_id) && store.len() >= MAX_CANVAS_COUNT {
            return None;
        }

        let entry = store
            .entry(canvas_id.to_string())
            .or_insert_with(|| CanvasEntry {
                current: None,
                history: Vec::new(),
                tx: broadcast::channel(BROADCAST_CAPACITY).0,
            });
        Some(entry.tx.subscribe())
    }

    /// List all canvas IDs that currently have content.
    pub fn list(&self) -> Vec<String> {
        let store = self.inner.read();
        store.keys().cloned().collect()
    }
}

/// `CanvasTool` — agent-callable tool for the Live Canvas (A2UI) system.
pub struct CanvasTool {
    store: CanvasStore,
    /// Workspace-root guard used to resolve/validate `content_file`. `None` in
    /// most tests and in any deployment with no workspace-scoping policy
    /// configured — in that case the guard degrades to the same fully-
    /// permissive posture `deliver_file` falls back to when it has no bounded
    /// allowlist root (null-byte / `..`-traversal rejection + canonicalize,
    /// no containment check).
    security: Option<Arc<SecurityPolicy>>,
}

impl CanvasTool {
    pub fn new(store: CanvasStore) -> Self {
        Self {
            store,
            security: None,
        }
    }

    /// Construct with a workspace-scoping [`SecurityPolicy`] so `content_file`
    /// reads are confined to the workspace, mirroring `deliver_file`'s guard.
    pub fn new_with_security(store: CanvasStore, security: Arc<SecurityPolicy>) -> Self {
        Self {
            store,
            security: Some(security),
        }
    }

    /// Resolve a caller-supplied `content_file`/`content_path` to a
    /// canonicalized, safe-to-read path. Mirrors `deliver_file`'s path guard:
    /// reject null bytes and `..` components up front, then canonicalize and
    /// (when a workspace-scoping [`SecurityPolicy`] is available) enforce
    /// containment within the workspace via `is_resolved_path_readable`.
    fn resolve_content_file(&self, path: &str) -> Result<std::path::PathBuf, String> {
        if path.contains('\0') {
            return Err("Path not allowed: contains null byte".to_string());
        }
        if std::path::Path::new(path)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("Path not allowed by security policy: {path}"));
        }

        match &self.security {
            Some(security) => {
                let candidate = security.resolve_tool_path(path);
                let resolved = std::fs::canonicalize(&candidate)
                    .map_err(|e| format!("Failed to resolve content_file path: {e}"))?;
                if !security.is_resolved_path_readable(&resolved) {
                    return Err(format!("Path escapes workspace directory: {path}"));
                }
                Ok(resolved)
            }
            None => std::fs::canonicalize(path)
                .map_err(|e| format!("Failed to resolve content_file path: {e}")),
        }
    }
}

/// Read a `content_file` for delivery through a directory handle bound to its
/// approved root (cap-std beneath/no-follow), enforcing `MAX_CONTENT_SIZE`.
/// Binding the open to the verified boundary — rather than re-walking
/// `resolved` by name via `std::fs::read` — means a file or a parent
/// component swapped to an escaping symlink after `resolve_content_file`'s
/// check cannot redirect the read outside the approved root. Mirrors
/// `deliver_file::read_source_bounded` exactly (down to the `None` fallback
/// for a fully-permissive/unconfigured policy).
fn read_content_file_bounded(
    security: Option<&SecurityPolicy>,
    resolved: &std::path::Path,
) -> Result<Vec<u8>, String> {
    use cap_std::ambient_authority;
    use cap_std::fs::Dir;

    match security.and_then(|s| s.approved_read_root(resolved)) {
        Some(root) => {
            let rel = resolved
                .strip_prefix(&root)
                .map_err(|_| "Path escapes its approved root".to_string())?;
            let dir = Dir::open_ambient_dir(&root, ambient_authority())
                .map_err(|e| format!("Failed to open approved root: {e}"))?;
            read_content_from_dir(&dir, rel)
        }
        None => {
            // No bounded allowlist root (no security policy configured, a
            // fully permissive policy, or a device path): there is no
            // confinement boundary to bind to. Open the final component
            // through a handle on its parent so at least a final-component
            // symlink escaping the parent is refused.
            let parent = resolved
                .parent()
                .ok_or_else(|| "Path has no parent directory".to_string())?;
            let name = resolved
                .file_name()
                .ok_or_else(|| "Path has no file name".to_string())?;
            let dir = Dir::open_ambient_dir(parent, ambient_authority())
                .map_err(|e| format!("Failed to open directory: {e}"))?;
            read_content_from_dir(&dir, std::path::Path::new(name))
        }
    }
}

/// Open `rel` beneath the already-opened directory handle `dir` (cap-std
/// refuses any component that escapes `dir` via `..` or an escaping symlink),
/// verify it is a regular file within `MAX_CONTENT_SIZE`, and return its
/// bytes. Every check and the read use that one opened handle, so nothing
/// swapped in at the pathname between check and read can redirect the bytes.
fn read_content_from_dir(dir: &cap_std::fs::Dir, rel: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let file = dir
        .open(rel)
        .map_err(|e| format!("Failed to open content_file: {e}"))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("Failed to read content_file metadata: {e}"))?;
    if !meta.is_file() {
        return Err("content_file is not a regular file".to_string());
    }
    if meta.len() > MAX_CONTENT_SIZE as u64 {
        return Err(format!(
            "Content exceeds maximum size of {} bytes",
            MAX_CONTENT_SIZE
        ));
    }
    // One extra byte over the cap catches a grow-after-stat race.
    let mut content = Vec::with_capacity(meta.len() as usize);
    file.take(MAX_CONTENT_SIZE as u64 + 1)
        .read_to_end(&mut content)
        .map_err(|e| format!("Failed to read content_file: {e}"))?;
    if content.len() > MAX_CONTENT_SIZE {
        return Err(format!(
            "Content exceeds maximum size of {} bytes",
            MAX_CONTENT_SIZE
        ));
    }
    Ok(content)
}

#[async_trait]
impl Tool for CanvasTool {
    fn name(&self) -> &str {
        "canvas"
    }

    fn description(&self) -> &str {
        "Push rendered content (HTML, SVG, Markdown) to a live web canvas that users can see \
         in real-time. Actions: render (push content), snapshot (get current content), \
         clear (reset canvas), eval (evaluate JS expression in canvas context). \
         Each canvas is identified by a canvas_id string."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action to perform on the canvas.",
                    "enum": ["render", "snapshot", "clear", "eval"]
                },
                "canvas_id": {
                    "type": "string",
                    "description": "Unique identifier for the canvas. Defaults to 'default'."
                },
                "content_type": {
                    "type": "string",
                    "description": "Content type for render action: html, svg, markdown, or text.",
                    "enum": ["html", "svg", "markdown", "text"]
                },
                "content": {
                    "type": "string",
                    "description": "Content to render (for render action). Mutually exclusive with content_file."
                },
                "content_file": {
                    "type": "string",
                    "description": "Workspace path to a file whose contents should be rendered \
                        (for render action), instead of passing large HTML inline as `content`. \
                        Read runtime-side, bounded by the same size cap as `content`. Mutually \
                        exclusive with `content`. Alias: content_path."
                },
                "content_path": {
                    "type": "string",
                    "description": "Alias for content_file."
                },
                "store": {
                    "type": "boolean",
                    "description": "For render action (default true): false = emit as an ACP \
                        ui:// artifact only, do not persist to the shared canvas store \
                        (session-isolated)."
                },
                "expression": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (for eval action). \
                        The result is returned as text. Evaluated client-side in the canvas iframe."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let action = match args.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: ToolOutput::default(),
                    error: Some("Missing required parameter: action".to_string()),
                });
            }
        };

        let canvas_id = args
            .get("canvas_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match action {
            "render" => {
                let content_type = args
                    .get("content_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("html");

                let content_arg = args.get("content").and_then(|v| v.as_str());
                let content_file_arg = args
                    .get("content_file")
                    .or_else(|| args.get("content_path"))
                    .and_then(|v| v.as_str());

                let content: String = match (content_arg, content_file_arg) {
                    (Some(_), Some(_)) => {
                        return Ok(ToolResult {
                            success: false,
                            output: ToolOutput::default(),
                            error: Some(
                                "content and content_file are mutually exclusive".to_string(),
                            ),
                        });
                    }
                    (Some(c), None) => c.to_string(),
                    (None, Some(f)) => {
                        let path = match self.resolve_content_file(f) {
                            Ok(p) => p,
                            Err(e) => {
                                return Ok(ToolResult {
                                    success: false,
                                    output: ToolOutput::default(),
                                    error: Some(e),
                                });
                            }
                        };
                        // Read through a cap-std Dir handle bound to the approved
                        // root (or its parent, when no policy scopes it) rather
                        // than re-walking `path` by name — closes the check/open
                        // TOCTOU a plain `std::fs::read(&path)` would leave open
                        // if a component were swapped to an escaping symlink
                        // between `resolve_content_file`'s check and this read.
                        // The 256 KiB cap is enforced here, before UTF-8 decode.
                        let bytes = match read_content_file_bounded(self.security.as_deref(), &path)
                        {
                            Ok(b) => b,
                            Err(e) => {
                                return Ok(ToolResult {
                                    success: false,
                                    output: ToolOutput::default(),
                                    error: Some(e),
                                });
                            }
                        };
                        match String::from_utf8(bytes) {
                            Ok(s) => s,
                            Err(e) => {
                                return Ok(ToolResult {
                                    success: false,
                                    output: ToolOutput::default(),
                                    error: Some(format!("content_file is not valid UTF-8: {e}")),
                                });
                            }
                        }
                    }
                    (None, None) => {
                        return Ok(ToolResult {
                            success: false,
                            output: ToolOutput::default(),
                            error: Some(
                                "Missing required parameter: content (for render action)"
                                    .to_string(),
                            ),
                        });
                    }
                };
                let content = content.as_str();

                if content.len() > MAX_CONTENT_SIZE {
                    return Ok(ToolResult {
                        success: false,
                        output: ToolOutput::default(),
                        error: Some(format!(
                            "Content exceeds maximum size of {} bytes",
                            MAX_CONTENT_SIZE
                        )),
                    });
                }

                let persist = args.get("store").and_then(|v| v.as_bool()).unwrap_or(true);

                let frame_id = if persist {
                    match self.store.render(canvas_id, content_type, content) {
                        Some(frame) => Some(frame.frame_id),
                        None => {
                            return Ok(ToolResult {
                                success: false,
                                output: ToolOutput::default(),
                                error: Some(format!(
                                    "Maximum canvas count ({}) reached. Clear unused canvases \
                                     first.",
                                    MAX_CANVAS_COUNT
                                )),
                            });
                        }
                    }
                } else {
                    None
                };

                if content_type == "html" {
                    let summary =
                        format!("Rendered ui://pnl/{canvas_id} ({} bytes).", content.len());
                    let data = json!({
                        "ui_resource": true,
                        "uri": format!("ui://pnl/{canvas_id}"),
                        "mimeType": "text/html",
                        "text": content,
                    });
                    return Ok(ToolResult {
                        success: true,
                        output: ToolOutput::json_with_text(data, summary),
                        error: None,
                    });
                }

                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Rendered {} content to canvas '{}'{}",
                        content_type,
                        canvas_id,
                        match &frame_id {
                            Some(id) => format!(" (frame: {id})"),
                            None => " (not persisted; store=false)".to_string(),
                        }
                    )
                    .into(),
                    error: None,
                })
            }

            // `snapshot` reflects ONLY content explicitly persisted with
            // `store: true` (the single-presenter/gateway use case). It is
            // NOT a cross-session read channel: an ACP render done with
            // `store: false` (see the `render` branch above) writes nothing
            // here, so a different session polling the same `canvas_id` sees
            // nothing of it (isolation §5a). There is no session id on this
            // tool to scope reads further — the only isolation guarantee is
            // "unstored content never appears in any snapshot, anywhere".
            "snapshot" => match self.store.snapshot(canvas_id) {
                Some(frame) => Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&frame)
                        .unwrap_or_else(|_| frame.content.clone())
                        .into(),
                    error: None,
                }),
                None => Ok(ToolResult {
                    success: true,
                    output: format!("Canvas '{}' is empty", canvas_id).into(),
                    error: None,
                }),
            },

            "clear" => {
                let existed = self.store.clear(canvas_id);
                Ok(ToolResult {
                    success: true,
                    output: if existed {
                        format!("Canvas '{}' cleared", canvas_id).into()
                    } else {
                        format!("Canvas '{}' was already empty", canvas_id).into()
                    },
                    error: None,
                })
            }

            "eval" => {
                // Eval is handled client-side. We store an eval request as a special frame
                // that the web viewer interprets.
                let expression = match args.get("expression").and_then(|v| v.as_str()) {
                    Some(e) => e,
                    None => {
                        return Ok(ToolResult {
                            success: false,
                            output: ToolOutput::default(),
                            error: Some(
                                "Missing required parameter: expression (for eval action)"
                                    .to_string(),
                            ),
                        });
                    }
                };

                // Push a special eval frame so connected clients know to evaluate it.
                match self.store.render(canvas_id, "eval", expression) {
                    Some(frame) => Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Eval request sent to canvas '{}' (frame: {}). \
                             Result will be available to connected viewers.",
                            canvas_id, frame.frame_id
                        )
                        .into(),
                        error: None,
                    }),
                    None => Ok(ToolResult {
                        success: false,
                        output: ToolOutput::default(),
                        error: Some(format!(
                            "Maximum canvas count ({}) reached. Clear unused canvases first.",
                            MAX_CANVAS_COUNT
                        )),
                    }),
                }
            }

            other => Ok(ToolResult {
                success: false,
                output: ToolOutput::default(),
                error: Some(format!(
                    "Unknown action: '{}'. Valid actions: render, snapshot, clear, eval",
                    other
                )),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_store_render_and_snapshot() {
        let store = CanvasStore::new();
        let frame = store.render("test", "html", "<h1>Hello</h1>").unwrap();
        assert_eq!(frame.content_type, "html");
        assert_eq!(frame.content, "<h1>Hello</h1>");

        let snapshot = store.snapshot("test").unwrap();
        assert_eq!(snapshot.frame_id, frame.frame_id);
        assert_eq!(snapshot.content, "<h1>Hello</h1>");
    }

    #[test]
    fn canvas_store_snapshot_empty_returns_none() {
        let store = CanvasStore::new();
        assert!(store.snapshot("nonexistent").is_none());
    }

    #[tokio::test]
    async fn canvas_tool_renders_into_shared_store() {
        let gateway_store = CanvasStore::new();
        let tool = CanvasTool::new(gateway_store.clone());

        let result = tool
            .execute(json!({
                "action": "render",
                "canvas_id": "default",
                "content_type": "html",
                "content": "<h1>from chat</h1>",
            }))
            .await
            .unwrap();
        assert!(result.success, "render failed: {:?}", result.error);

        let seen = gateway_store
            .snapshot("default")
            .expect("frame must be visible via the shared gateway store");
        assert_eq!(seen.content, "<h1>from chat</h1>");
    }

    #[test]
    fn canvas_store_clear_removes_content() {
        let store = CanvasStore::new();
        store.render("test", "html", "<p>content</p>");
        assert!(store.snapshot("test").is_some());

        let cleared = store.clear("test");
        assert!(cleared);
        assert!(store.snapshot("test").is_none());
    }

    #[test]
    fn canvas_store_clear_nonexistent_returns_false() {
        let store = CanvasStore::new();
        assert!(!store.clear("nonexistent"));
    }

    #[test]
    fn canvas_store_history_tracks_frames() {
        let store = CanvasStore::new();
        store.render("test", "html", "frame1");
        store.render("test", "html", "frame2");
        store.render("test", "html", "frame3");

        let history = store.history("test");
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].content, "frame1");
        assert_eq!(history[2].content, "frame3");
    }

    #[test]
    fn canvas_store_history_limit_enforced() {
        let store = CanvasStore::new();
        for i in 0..60 {
            store.render("test", "html", &format!("frame{i}"));
        }

        let history = store.history("test");
        assert_eq!(history.len(), MAX_HISTORY_FRAMES);
        // Oldest frames should have been dropped
        assert_eq!(history[0].content, "frame10");
    }

    #[test]
    fn canvas_store_list_returns_canvas_ids() {
        let store = CanvasStore::new();
        store.render("alpha", "html", "a");
        store.render("beta", "svg", "b");

        let mut ids = store.list();
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[test]
    fn canvas_store_subscribe_receives_updates() {
        let store = CanvasStore::new();
        let mut rx = store.subscribe("test").unwrap();
        store.render("test", "html", "<p>live</p>");

        let frame = rx.try_recv().unwrap();
        assert_eq!(frame.content, "<p>live</p>");
    }

    #[tokio::test]
    async fn canvas_tool_render_action() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store.clone());
        let result = tool
            .execute(json!({
                "action": "render",
                "canvas_id": "test",
                "content_type": "html",
                "content": "<h1>Hello World</h1>"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("ui://pnl/test"));

        let snapshot = store.snapshot("test").unwrap();
        assert_eq!(snapshot.content, "<h1>Hello World</h1>");
    }

    #[tokio::test]
    async fn canvas_tool_snapshot_action() {
        let store = CanvasStore::new();
        store.render("test", "html", "<p>snap</p>");
        let tool = CanvasTool::new(store);
        let result = tool
            .execute(json!({"action": "snapshot", "canvas_id": "test"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("<p>snap</p>"));
    }

    #[tokio::test]
    async fn canvas_tool_snapshot_empty() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store);
        let result = tool
            .execute(json!({"action": "snapshot", "canvas_id": "empty"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("empty"));
    }

    #[tokio::test]
    async fn canvas_tool_clear_action() {
        let store = CanvasStore::new();
        store.render("test", "html", "<p>clear me</p>");
        let tool = CanvasTool::new(store.clone());
        let result = tool
            .execute(json!({"action": "clear", "canvas_id": "test"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("cleared"));
        assert!(store.snapshot("test").is_none());
    }

    #[tokio::test]
    async fn canvas_tool_eval_action() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store.clone());
        let result = tool
            .execute(json!({
                "action": "eval",
                "canvas_id": "test",
                "expression": "document.title"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("Eval request sent"));

        let snapshot = store.snapshot("test").unwrap();
        assert_eq!(snapshot.content_type, "eval");
        assert_eq!(snapshot.content, "document.title");
    }

    #[tokio::test]
    async fn canvas_tool_unknown_action() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store);
        let result = tool.execute(json!({"action": "invalid"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("Unknown action"));
    }

    #[tokio::test]
    async fn canvas_tool_missing_action() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store);
        let result = tool.execute(json!({})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("action"));
    }

    #[tokio::test]
    async fn canvas_tool_render_missing_content() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store);
        let result = tool
            .execute(json!({"action": "render", "canvas_id": "test"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("content"));
    }

    #[tokio::test]
    async fn canvas_tool_render_content_too_large() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store);
        let big_content = "x".repeat(MAX_CONTENT_SIZE + 1);
        let result = tool
            .execute(json!({
                "action": "render",
                "canvas_id": "test",
                "content": big_content
            }))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("maximum size"));
    }

    #[tokio::test]
    async fn canvas_tool_default_canvas_id() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store.clone());
        let result = tool
            .execute(json!({
                "action": "render",
                "content_type": "html",
                "content": "<p>default</p>"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert!(store.snapshot("default").is_some());
    }

    #[test]
    fn canvas_store_enforces_max_canvas_count() {
        let store = CanvasStore::new();
        // Create MAX_CANVAS_COUNT canvases
        for i in 0..MAX_CANVAS_COUNT {
            assert!(
                store
                    .render(&format!("canvas_{i}"), "html", "content")
                    .is_some()
            );
        }
        // The next new canvas should be rejected
        assert!(store.render("one_too_many", "html", "content").is_none());
        // But rendering to an existing canvas should still work
        assert!(store.render("canvas_0", "html", "updated").is_some());
    }

    #[tokio::test]
    async fn render_html_returns_ui_resource_output_data() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store.clone());
        let out = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "dashboard",
                "content_type": "html", "content": "<!doctype html><title>d</title>"
            }))
            .await
            .unwrap();
        let data = out.output.into_data().expect("has data");
        assert_eq!(data["ui_resource"], serde_json::json!(true));
        assert_eq!(data["uri"], serde_json::json!("ui://pnl/dashboard"));
        assert_eq!(data["mimeType"], serde_json::json!("text/html"));
        assert!(data["text"].as_str().unwrap().starts_with("<!doctype"));
    }

    #[tokio::test]
    async fn render_with_store_false_does_not_persist() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store.clone());
        let _ = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "dashboard", "store": false,
                "content_type": "html", "content": "<!doctype html><title>secret</title>"
            }))
            .await
            .unwrap();
        // nothing persisted → snapshot/current for this id is empty (isolation §5a)
        assert!(
            store.current("dashboard").is_none(),
            "ACP render must not write shared store"
        );
    }

    /// Isolation acceptance (§5a): an ACP render done with `store: false` in
    /// one session must never be readable back via `snapshot` from another
    /// session sharing the same `CanvasStore`. Two `CanvasTool`s built on one
    /// `store.clone()` stand in for "two sessions" — there is no per-session
    /// scoping on `CanvasStore`, so the only guarantee available is that
    /// unstored content is written nowhere at all.
    #[tokio::test]
    async fn snapshot_cannot_read_back_acp_render() {
        let store = CanvasStore::new(); // one shared store == "two sessions"
        let a = CanvasTool::new(store.clone()); // session A
        let b = CanvasTool::new(store.clone()); // session B

        // A renders an ACP artifact (store:false)
        let _ = a
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "pnl/dashboard", "store": false,
                "content_type": "html", "content": "<!doctype html><title>A-private</title>"
            }))
            .await
            .unwrap();

        // B tries to read it back by bare id
        let snap = b
            .execute(serde_json::json!({
                "action": "snapshot", "canvas_id": "pnl/dashboard"
            }))
            .await
            .unwrap();

        assert!(
            !snap.output.contains("A-private"),
            "session B must not read session A's ACP render"
        );
    }

    #[tokio::test]
    async fn canvas_tool_eval_missing_expression() {
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store);
        let result = tool
            .execute(json!({"action": "eval", "canvas_id": "test"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("expression"));
    }

    #[tokio::test]
    async fn render_reads_content_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("dash.html");
        std::fs::write(&p, "<!doctype html><title>fromfile</title>").unwrap();
        let store = CanvasStore::new();
        let tool = CanvasTool::new(store); // no security policy: permissive fallback guard
        let out = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "dashboard", "content_type": "html",
                "content_file": p.to_string_lossy()
            }))
            .await
            .unwrap();
        let data = out.output.into_data().unwrap();
        assert!(data["text"].as_str().unwrap().contains("fromfile"));
    }

    #[tokio::test]
    async fn render_rejects_content_and_content_file_together() {
        let tool = CanvasTool::new(CanvasStore::new());
        let err = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "d", "content_type": "html",
                "content": "<x>", "content_file": "y.html"
            }))
            .await;
        assert!(err.is_err() || !err.unwrap().success, "mutually exclusive");
    }

    #[tokio::test]
    async fn render_content_file_alias_content_path_works() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("alias.html");
        std::fs::write(&p, "<!doctype html><title>viapath</title>").unwrap();
        let tool = CanvasTool::new(CanvasStore::new());
        let out = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "d", "content_type": "html",
                "content_path": p.to_string_lossy()
            }))
            .await
            .unwrap();
        assert!(out.success, "error: {:?}", out.error);
        let data = out.output.into_data().unwrap();
        assert!(data["text"].as_str().unwrap().contains("viapath"));
    }

    #[tokio::test]
    async fn render_content_file_missing_file_errors() {
        let tool = CanvasTool::new(CanvasStore::new());
        let result = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "d", "content_type": "html",
                "content_file": "/nonexistent/path/does-not-exist.html"
            }))
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn render_content_file_scoped_by_security_policy_rejects_escape() {
        use zeroclaw_config::policy::{AutonomyLevel, SecurityPolicy};

        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("secret.html");
        std::fs::write(&outside_file, "<!doctype html><title>secret</title>").unwrap();

        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: workspace.path().to_path_buf(),
            ..SecurityPolicy::default()
        });
        let tool = CanvasTool::new_with_security(CanvasStore::new(), security);
        let result = tool
            .execute(serde_json::json!({
                "action": "render", "canvas_id": "d", "content_type": "html",
                "content_file": outside_file.to_string_lossy()
            }))
            .await
            .unwrap();
        assert!(
            !result.success,
            "content_file outside the workspace must be rejected when a security policy scopes it"
        );
    }
}
