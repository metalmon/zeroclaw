use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolOutput, ToolResult};

const MAX_DELIVER_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Deliver a workspace file to an ACP client as an embedded binary resource.
///
/// Returns path/mime metadata (and a machine trailer for ACP) without embedding
/// file bytes in the tool result — the ACP layer re-reads the file for `blob`.
pub struct DeliverFileTool {
    security: Arc<SecurityPolicy>,
}

impl DeliverFileTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn resolve_candidate(&self, path: &str) -> anyhow::Result<std::path::PathBuf> {
        if path.contains('\0') {
            anyhow::bail!("Path not allowed: contains null byte");
        }
        if std::path::Path::new(path)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            anyhow::bail!("Path not allowed by security policy: {path}");
        }

        Ok(self.security.resolve_tool_path(path))
    }

    fn mime_for(path: &std::path::Path, explicit: Option<&str>) -> String {
        if let Some(mime) = explicit.filter(|m| !m.is_empty()) {
            return mime.to_string();
        }
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string()
    }
}

#[async_trait]
impl Tool for DeliverFileTool {
    fn name(&self) -> &str {
        "deliver_file"
    }

    fn description(&self) -> &str {
        "Deliver a file from the workspace to the ACP client as an embedded binary resource \
         (PDF, DOCX, images, etc.). Use when the user should download or preview the file. \
         Path must stay inside the workspace."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative or absolute path inside the workspace"
                },
                "mimeType": {
                    "type": "string",
                    "description": "Optional MIME type; guessed from extension if omitted"
                }
            },
            "required": ["path"]
        })
    }

    fn output_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "delivered": { "type": "boolean" },
                "path": { "type": "string" },
                "filename": { "type": "string" },
                "mimeType": { "type": "string" },
                "bytes": { "type": "integer" }
            }
        }))
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            anyhow::Error::msg("Missing 'path' parameter")
        })?;

        let full_path = match self.resolve_candidate(path) {
            Ok(p) => p,
            Err(e) => {
                let _ = self.security.record_action();
                return Ok(ToolResult {
                    success: false,
                    output: ToolOutput::default(),
                    error: Some(e.to_string()),
                });
            }
        };

        let resolved_path = match tokio::fs::canonicalize(&full_path).await {
            Ok(p) => p,
            Err(e) => {
                let _ = self.security.record_action();
                return Ok(ToolResult {
                    success: false,
                    output: ToolOutput::default(),
                    error: Some(format!("Failed to resolve file path: {e}")),
                });
            }
        };

        if !self.security.is_resolved_path_readable(&resolved_path) {
            return Ok(ToolResult {
                success: false,
                output: ToolOutput::default(),
                error: Some(format!("Path escapes workspace directory: {path}")),
            });
        }

        let meta = match tokio::fs::metadata(&resolved_path).await {
            Ok(meta) => meta,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: ToolOutput::default(),
                    error: Some(format!("Failed to read file metadata: {e}")),
                });
            }
        };

        if !meta.is_file() {
            return Ok(ToolResult {
                success: false,
                output: ToolOutput::default(),
                error: Some(format!("Not a file: {path}")),
            });
        }

        if meta.len() > MAX_DELIVER_FILE_BYTES {
            return Ok(ToolResult {
                success: false,
                output: ToolOutput::default(),
                error: Some(format!(
                    "File too large: {} bytes (limit: {MAX_DELIVER_FILE_BYTES} bytes)",
                    meta.len()
                )),
            });
        }

        // Ensure the file is readable (ACP will re-read for the blob).
        if let Err(e) = tokio::fs::read(&resolved_path).await {
            return Ok(ToolResult {
                success: false,
                output: ToolOutput::default(),
                error: Some(format!("Failed to read file: {e}")),
            });
        }

        let filename = resolved_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let mime_type = Self::mime_for(
            &resolved_path,
            args.get("mimeType").and_then(|v| v.as_str()),
        );
        let abs_path = resolved_path.to_string_lossy().to_string();
        let bytes = meta.len();

        let summary = format!(
            "Delivered {filename} ({bytes} bytes)\nacp.deliver_file path={abs_path} mimeType={mime_type}"
        );
        let data = json!({
            "delivered": true,
            "path": abs_path,
            "filename": filename,
            "mimeType": mime_type,
            "bytes": bytes,
        });

        let _ = self.security.record_action();
        Ok(ToolResult {
            success: true,
            output: ToolOutput::json_with_text(data, summary),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{AutonomyLevel, SecurityPolicy};

    fn test_tool(workspace: std::path::PathBuf) -> DeliverFileTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: workspace,
            ..SecurityPolicy::default()
        });
        DeliverFileTool::new(security)
    }

    #[tokio::test]
    async fn delivers_json_with_path_and_mime() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.pdf");
        std::fs::write(&file, b"%PDF-1.4").unwrap();
        let tool = test_tool(dir.path().to_path_buf());
        let result = tool
            .execute(json!({"path": "a.pdf", "mimeType": "application/pdf"}))
            .await
            .unwrap();
        assert!(result.success);
        let data = result.output.data().expect("structured data");
        assert_eq!(data["mimeType"], "application/pdf");
        assert!(data["path"].as_str().unwrap().contains("a.pdf"));
        assert_eq!(data["filename"], "a.pdf");
        assert_eq!(data["bytes"], 8);
        let text = result.output.as_str();
        assert!(text.contains("Delivered a.pdf"));
        assert!(text.contains("acp.deliver_file path="));
        assert!(text.contains("mimeType=application/pdf"));
    }

    #[tokio::test]
    async fn guesses_mime_when_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.txt");
        std::fs::write(&file, b"hi").unwrap();
        let tool = test_tool(dir.path().to_path_buf());
        let result = tool.execute(json!({"path": "note.txt"})).await.unwrap();
        assert!(result.success);
        let data = result.output.data().unwrap();
        assert_eq!(data["mimeType"], "text/plain");
    }

    #[tokio::test]
    async fn rejects_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        let tool = test_tool(dir.path().to_path_buf());
        let result = tool
            .execute(json!({"path": "../outside.txt"}))
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn rejects_oversized_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.bin");
        let oversized = vec![0u8; (MAX_DELIVER_FILE_BYTES as usize) + 1];
        std::fs::write(&file, &oversized).unwrap();
        let tool = test_tool(dir.path().to_path_buf());
        let result = tool.execute(json!({"path": "big.bin"})).await.unwrap();
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("File too large")
        );
    }

    #[tokio::test]
    async fn success_json_includes_attachment_deliver_uri() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a1b2c3d4e5f6.pdf");
        std::fs::write(&file, b"%PDF-1.4").unwrap();
        let tool = test_tool(dir.path().to_path_buf());
        let result = tool
            .execute(json!({"path": "a1b2c3d4e5f6.pdf", "mimeType": "application/pdf"}))
            .await
            .unwrap();
        assert!(result.success);
        let data = result.output.data().expect("structured data");
        assert_eq!(
            data["uri"].as_str().unwrap(),
            "attachment://deliver/a1b2c3d4e5f6.pdf"
        );
        let text = result.output.as_str();
        assert!(
            text.contains("uri=attachment://deliver/a1b2c3d4e5f6.pdf"),
            "summary must carry uri for models that skim text: {text}"
        );
    }

    #[tokio::test]
    async fn failure_omits_success_uri() {
        let dir = tempfile::tempdir().unwrap();
        let tool = test_tool(dir.path().to_path_buf());
        let result = tool
            .execute(json!({"path": "../outside.txt"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.output.data().is_none());
        assert!(!result.output.as_str().contains("attachment://deliver/"));
    }

    #[test]
    fn attachment_deliver_uri_helper_formats_basename() {
        assert_eq!(
            attachment_deliver_uri("report.pdf"),
            "attachment://deliver/report.pdf"
        );
    }
}
