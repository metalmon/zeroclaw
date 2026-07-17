//! Materialize embedded `resource.blob` payloads into the session workspace.
//! Store-agnostic: no RPC `SessionStore` / `file/attach`. Shared by ACP inbound
//! and MCP tools/call postprocessing.

use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Per-file decoded size limit for embedded blobs (matches RPC attach / ACP).
pub const MAX_EMBEDDED_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Result of writing an embedded resource into the session workspace.
#[derive(Debug)]
pub struct MaterializedResource {
    pub abs_path: PathBuf,
    pub marker: String,
    pub mime_type: String,
    pub filename: String,
}

/// Error while decoding or persisting an embedded blob.
#[derive(Debug)]
pub struct EmbeddedResourceError(pub String);

impl std::fmt::Display for EmbeddedResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EmbeddedResourceError {}

/// Decode `blob_b64`, enforce size limits, write under `{workspace}/uploads/`,
/// and return a prompt marker (`[Document: …]` or `[IMAGE:…]`).
pub fn materialize_resource_blob(
    workspace_dir: &Path,
    uri: Option<&str>,
    mime_type: Option<&str>,
    blob_b64: &str,
) -> Result<MaterializedResource, EmbeddedResourceError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob_b64.trim())
        .map_err(|e| EmbeddedResourceError(format!("Invalid base64: {e}")))?;

    if bytes.len() as u64 > MAX_EMBEDDED_FILE_BYTES {
        return Err(EmbeddedResourceError(format!(
            "Embedded resource exceeds {} MB limit ({} bytes)",
            MAX_EMBEDDED_FILE_BYTES / (1024 * 1024),
            bytes.len()
        )));
    }

    let filename = sanitize_filename(&filename_from_uri(uri));
    let mime = mime_type
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| mime_from_filename(&filename));

    let hex = format!("{:x}", Sha256::digest(&bytes));
    let ext = Path::new(&filename)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    let storage_name = if ext.is_empty() {
        hex[..16].to_string()
    } else {
        format!("{}.{ext}", &hex[..16])
    };

    let upload_dir = workspace_dir.join("uploads");
    std::fs::create_dir_all(&upload_dir).map_err(|e| {
        EmbeddedResourceError(format!("Cannot create upload dir: {e}"))
    })?;
    let dest = upload_dir.join(&storage_name);

    let needs_write = match std::fs::metadata(&dest) {
        Ok(meta) => meta.len() != bytes.len() as u64,
        Err(_) => true,
    };
    if needs_write {
        std::fs::write(&dest, &bytes)
            .map_err(|e| EmbeddedResourceError(format!("Cannot write upload: {e}")))?;
    }

    let abs_path = std::fs::canonicalize(&dest).unwrap_or(dest);
    let abs_display = strip_windows_verbatim_prefix(&abs_path.to_string_lossy()).into_owned();
    let marker = if mime.starts_with("image/") {
        format!("[IMAGE:{abs_display}]")
    } else {
        format!("[Document: {filename}] {abs_display}")
    };

    Ok(MaterializedResource {
        abs_path,
        marker,
        mime_type: mime,
        filename,
    })
}

/// Whether an MCP tools/call content item is a `resource` with a `blob` field.
pub fn content_item_has_resource_blob(item: &serde_json::Value) -> bool {
    item.get("type").and_then(|t| t.as_str()) == Some("resource")
        && item
            .get("resource")
            .and_then(|r| r.get("blob"))
            .and_then(|b| b.as_str())
            .is_some()
}

/// Format an MCP `tools/call` result for the model.
///
/// When `content` contains any `type: "resource"` item with `blob`, materialize
/// each blob under `{workspace}/uploads/`, and return provenance text (non-blob
/// content) plus Document/IMAGE markers — never the raw base64. Results without
/// resource blobs keep the existing pretty-printed JSON shape.
pub fn format_mcp_tool_result_for_model(
    result: &serde_json::Value,
    workspace_dir: &Path,
) -> Result<String, EmbeddedResourceError> {
    let Some(content) = result.get("content").and_then(|c| c.as_array()) else {
        return Ok(serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()));
    };

    if !content.iter().any(content_item_has_resource_blob) {
        return Ok(serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()));
    }

    let mut parts: Vec<String> = Vec::new();
    for item in content {
        let typ = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match typ {
            "text" => {
                if let Some(text) = item.get("text").and_then(|v| v.as_str())
                    && !text.is_empty()
                {
                    parts.push(text.to_string());
                }
            }
            "resource" => {
                let Some(res) = item.get("resource") else {
                    continue;
                };
                if let Some(blob) = res.get("blob").and_then(|b| b.as_str()) {
                    if let Some(text) = res.get("text").and_then(|v| v.as_str())
                        && !text.is_empty()
                    {
                        parts.push(text.to_string());
                    }
                    let uri = res.get("uri").and_then(|v| v.as_str());
                    let mime = res.get("mimeType").and_then(|v| v.as_str());
                    let materialized =
                        materialize_resource_blob(workspace_dir, uri, mime, blob)?;
                    parts.push(materialized.marker);
                } else if let Some(text) = res.get("text").and_then(|v| v.as_str())
                    && !text.is_empty()
                {
                    parts.push(text.to_string());
                }
            }
            _ => {
                // Leave other content types out of the rewritten surface so we
                // never dump sibling base64 payloads alongside materialized blobs.
            }
        }
    }

    if parts.is_empty() {
        Ok(String::new())
    } else {
        Ok(parts.join("\n\n"))
    }
}

fn filename_from_uri(uri: Option<&str>) -> String {
    let Some(uri) = uri.map(str::trim).filter(|s| !s.is_empty()) else {
        return "upload.bin".to_string();
    };
    let without_scheme = uri
        .strip_prefix("file://")
        .or_else(|| uri.strip_prefix("attachment://"))
        .unwrap_or(uri);
    let name = without_scheme
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(without_scheme)
        .trim();
    if name.is_empty() || name == "." || name == ".." {
        "upload.bin".to_string()
    } else {
        name.to_string()
    }
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let sanitized = sanitized.replace("..", "_");
    if sanitized.is_empty() {
        "upload.bin".to_string()
    } else {
        sanitized
    }
}

fn mime_from_filename(filename: &str) -> String {
    match Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png".into(),
        Some("jpg" | "jpeg") => "image/jpeg".into(),
        Some("gif") => "image/gif".into(),
        Some("webp") => "image/webp".into(),
        Some("pdf") => "application/pdf".into(),
        Some("docx") => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into()
        }
        Some("doc") => "application/msword".into(),
        Some("txt") => "text/plain".into(),
        Some("md") => "text/markdown".into(),
        Some("json") => "application/json".into(),
        _ => "application/octet-stream".into(),
    }
}

fn strip_windows_verbatim_prefix(path: &str) -> std::borrow::Cow<'_, str> {
    path.strip_prefix(r"\\?\")
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| std::borrow::Cow::Borrowed(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn writes_blob_under_uploads_and_returns_document_marker() {
        let dir = tempdir().unwrap();
        let bytes = b"hello docx";
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
        let out = materialize_resource_blob(
            dir.path(),
            Some("file:///x/report.docx"),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            &b64,
        )
        .unwrap();
        assert!(out.abs_path.exists());
        assert!(out.marker.contains("[Document: report.docx]"));
        assert!(
            out.marker
                .contains(out.abs_path.to_string_lossy().as_ref())
                || out.marker.contains(
                    strip_windows_verbatim_prefix(&out.abs_path.to_string_lossy()).as_ref()
                )
        );
        assert_eq!(std::fs::read(&out.abs_path).unwrap(), bytes);
    }

    #[test]
    fn image_mime_uses_image_marker() {
        let dir = tempdir().unwrap();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"img");
        let out =
            materialize_resource_blob(dir.path(), Some("file:///a.png"), Some("image/png"), &b64)
                .unwrap();
        assert!(out.marker.starts_with("[IMAGE:"));
    }

    #[test]
    fn rejects_invalid_base64() {
        let dir = tempdir().unwrap();
        let err = materialize_resource_blob(dir.path(), None, None, "%%%").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("base64"));
    }

    #[test]
    fn rejects_oversized_blob() {
        let dir = tempdir().unwrap();
        let big = vec![0u8; (MAX_EMBEDDED_FILE_BYTES as usize) + 1];
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &big);
        let err =
            materialize_resource_blob(dir.path(), Some("file:///big.bin"), None, &b64).unwrap_err();
        assert!(err.to_string().contains("MB") || err.to_string().contains("limit"));
    }

    #[test]
    fn mcp_intake_materializes_blob_and_omits_base64() {
        let dir = tempdir().unwrap();
        let bytes = b"%PDF-1.4 fake";
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
        let result = json!({
            "content": [
                { "type": "text", "text": "Fetched original" },
                {
                    "type": "resource",
                    "resource": {
                        "uri": "file:///kb/report.pdf",
                        "mimeType": "application/pdf",
                        "blob": b64,
                    }
                }
            ]
        });
        let out = format_mcp_tool_result_for_model(&result, dir.path()).unwrap();
        assert!(out.contains("Fetched original"));
        assert!(out.contains("[Document: report.pdf]"));
        assert!(!out.contains(&b64), "base64 must not reach the model: {out}");
        let uploads = dir.path().join("uploads");
        assert!(uploads.exists());
        let entries: Vec<_> = std::fs::read_dir(&uploads).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let path = entries[0].as_ref().unwrap().path();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn mcp_intake_image_blob_uses_image_marker() {
        let dir = tempdir().unwrap();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"img");
        let result = json!({
            "content": [{
                "type": "resource",
                "resource": {
                    "uri": "file:///a.png",
                    "mimeType": "image/png",
                    "blob": b64,
                }
            }]
        });
        let out = format_mcp_tool_result_for_model(&result, dir.path()).unwrap();
        assert!(out.starts_with("[IMAGE:"));
        assert!(!out.contains(&b64));
    }

    #[test]
    fn mcp_intake_rejects_bad_base64() {
        let dir = tempdir().unwrap();
        let result = json!({
            "content": [{
                "type": "resource",
                "resource": {
                    "uri": "file:///x.bin",
                    "blob": "%%%",
                }
            }]
        });
        let err = format_mcp_tool_result_for_model(&result, dir.path()).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("base64"));
    }

    #[test]
    fn mcp_intake_rejects_oversized_blob() {
        let dir = tempdir().unwrap();
        let big = vec![0u8; (MAX_EMBEDDED_FILE_BYTES as usize) + 1];
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &big);
        let result = json!({
            "content": [{
                "type": "resource",
                "resource": {
                    "uri": "file:///big.bin",
                    "blob": b64,
                }
            }]
        });
        let err = format_mcp_tool_result_for_model(&result, dir.path()).unwrap_err();
        assert!(err.to_string().contains("MB") || err.to_string().contains("limit"));
    }

    #[test]
    fn mcp_intake_without_blob_keeps_pretty_json() {
        let dir = tempdir().unwrap();
        let result = json!({
            "content": [{ "type": "text", "text": "plain" }]
        });
        let out = format_mcp_tool_result_for_model(&result, dir.path()).unwrap();
        assert!(out.contains("\"type\": \"text\"") || out.contains("\"type\":\"text\""));
        assert!(out.contains("plain"));
        assert!(!dir.path().join("uploads").exists());
    }

    #[test]
    fn mcp_intake_gates_on_shape_not_tool_name() {
        // Shape gate: resource+blob is enough; no tool-name checks.
        assert!(content_item_has_resource_blob(&json!({
            "type": "resource",
            "resource": { "uri": "u", "blob": "YQ==" }
        })));
        assert!(!content_item_has_resource_blob(&json!({
            "type": "resource",
            "resource": { "uri": "u", "text": "hi" }
        })));
        assert!(!content_item_has_resource_blob(&json!({
            "type": "text",
            "text": "hi"
        })));
    }
}
