# ACP Embedded Resource Blob (P0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ZeroClaw ACP accepts inbound `resource.blob`, advertises `embeddedContext: true`, and can outbound-deliver workspace files via `deliver_file` as standard ACP `resource`+`blob` content (P0 only; no ACP Image/Audio ContentBlocks yet).

**Architecture:** A small store-agnostic helper materializes blobs under `{session.workspaceDir}/uploads/`. Prompt intake in `handle_session_prompt` uses that helper. A new `deliver_file` tool returns JSON (path + mime); ACP `notification_for_turn_event` special-cases that tool name, re-reads the file, and emits ACP resource content without putting base64 in `rawOutput`.

**Tech Stack:** Rust, `zeroclaw-channels` ACP server, `zeroclaw-runtime` tools, `base64`/`sha2`, existing `SecurityPolicy` jail, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-07-17-acp-embedded-resource-blob-design.md`

---

## File map

| File | Responsibility |
|------|----------------|
| `crates/zeroclaw-channels/src/orchestrator/acp_embedded.rs` | Decode blob, size limit, sha path, write uploads, build markers |
| `crates/zeroclaw-channels/src/orchestrator/mod.rs` | `mod acp_embedded` (cfg with acp-server) |
| `crates/zeroclaw-channels/src/orchestrator/acp_server.rs` | `embeddedContext: true`; materialize blobs in prompt path; deliver_file ACP content |
| `crates/zeroclaw-runtime/src/tools/deliver_file.rs` | New tool: jail + read + JSON result |
| `crates/zeroclaw-runtime/src/tools/mod.rs` | Register tool in default toolsets |
| `docs/book/src/channels/acp.md` | Document capability, blob prompt, deliver_file |

---

### Task 1: Shared embedded-resource helper + unit tests

**Files:**
- Create: `crates/zeroclaw-channels/src/orchestrator/acp_embedded.rs`
- Modify: `crates/zeroclaw-channels/src/orchestrator/mod.rs` — add `#[cfg(feature = "channel-acp-server")] pub mod acp_embedded;`

- [ ] **Step 1: Write the failing tests in `acp_embedded.rs`**

```rust
//! Materialize ACP embedded `resource.blob` into the session workspace.
//! Store-agnostic: no RPC `SessionStore` / `file/attach`.

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(out.marker.contains(out.abs_path.to_string_lossy().as_ref()));
        assert_eq!(std::fs::read(&out.abs_path).unwrap(), bytes);
    }

    #[test]
    fn image_mime_uses_image_marker() {
        let dir = tempdir().unwrap();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"img");
        let out = materialize_resource_blob(dir.path(), Some("file:///a.png"), Some("image/png"), &b64).unwrap();
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
        let err = materialize_resource_blob(dir.path(), Some("file:///big.bin"), None, &b64).unwrap_err();
        assert!(err.to_string().contains("MB") || err.to_string().contains("limit"));
    }
}
```

Also define (same file, above tests):

```rust
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const MAX_EMBEDDED_FILE_BYTES: u64 = 10 * 1024 * 1024;

pub struct MaterializedResource {
    pub abs_path: PathBuf,
    pub marker: String,
    pub mime_type: String,
    pub filename: String,
}

#[derive(Debug)]
pub struct EmbeddedResourceError(pub String);

impl std::fmt::Display for EmbeddedResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EmbeddedResourceError {}

pub fn materialize_resource_blob(
    workspace_dir: &Path,
    uri: Option<&str>,
    mime_type: Option<&str>,
    blob_b64: &str,
) -> Result<MaterializedResource, EmbeddedResourceError> {
    todo!("implement in step 3")
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p zeroclaw-channels --features channel-acp-server acp_embedded -- --nocapture
```

Expected: compile error or `todo!` panic / missing symbols.

- [ ] **Step 3: Implement `materialize_resource_blob`**

Logic:
1. `STANDARD.decode(blob_b64)` → map err to `EmbeddedResourceError`.
2. If `bytes.len() as u64 > MAX_EMBEDDED_FILE_BYTES` → error mentioning 10 MB.
3. Filename: basename of `uri` (strip `file://`, take last path segment), else `upload.bin`. Sanitize: replace `..`, `/`, `\`, NUL with `_`.
4. `mime_type`: arg or guess from extension (`png`→`image/png`, `pdf`→`application/pdf`, `docx`→OOXML mime, else `application/octet-stream`).
5. `hex = Sha256::digest(&bytes)` formatted lowercase; `storage = format!("{}.{}", &hex[..16], ext)` where `ext` from filename or empty.
6. `upload_dir = workspace_dir.join("uploads")`; `create_dir_all`; `dest = upload_dir.join(storage)`.
7. If `dest` missing or size differs, `fs::write`.
8. Canonicalize `dest` (fallback to dest); build marker:
   - if mime starts with `image/` → `[IMAGE:{abs}]`
   - else → `[Document: {filename}] {abs}`
9. Return `MaterializedResource`.

- [ ] **Step 4: Re-run tests**

Run same command. Expected: all `acp_embedded` tests PASS.

- [ ] **Step 5: Commit**

```bash
git add -f crates/zeroclaw-channels/src/orchestrator/acp_embedded.rs crates/zeroclaw-channels/src/orchestrator/mod.rs
git commit -m "$(cat <<'EOF'
feat(acp): add store-agnostic embedded resource blob helper

Materialize ACP resource.blob under session uploads/ with size limits and markers.
EOF
)"
```

(On Windows PowerShell, use a here-string for `-m` if `cat <<` is unavailable.)

---

### Task 2: Advertise `embeddedContext` + materialize blobs in `session/prompt`

**Files:**
- Modify: `crates/zeroclaw-channels/src/orchestrator/acp_server.rs`

- [ ] **Step 1: Update the initialize capability test to expect `true`**

Find `initialize_response_uses_acp_v1_shape` (or similar asserting `image`/`embeddedContext`). Change expectation:

```rust
assert_eq!(
    result["agentCapabilities"]["promptCapabilities"]["embeddedContext"],
    false // change to true
);
```

to `true`. Keep `image`/`audio` assertions as `false`.

- [ ] **Step 2: Run that test — expect FAIL**

```bash
cargo test -p zeroclaw-channels --features channel-acp-server initialize_response_uses_acp_v1_shape -- --nocapture
```

- [ ] **Step 3: Flip capability in `handle_initialize`**

In the JSON for `promptCapabilities`, set `"embeddedContext": true` (leave image/audio false).

- [ ] **Step 4: Add tests for blob prompt materialization**

Add tests near existing `parse_prompt` tests. Prefer testing a new function rather than the whole async handler:

Refactor `parse_prompt` into:

```rust
fn materialize_prompt(
    params: &Value,
    workspace_dir: Option<&Path>,
) -> Result<String, RpcError>
```

Behavior:
- String prompt: unchanged.
- Array parts:
  - append `text` as today;
  - for `resource.text`: append text as today;
  - for `resource.blob`: require `workspace_dir`; call `acp_embedded::materialize_resource_blob`; append marker; map `EmbeddedResourceError` → `RpcError { code: INVALID_PARAMS, message }`;
  - if both text and blob on same resource, append text then marker (blob marker after text, separated by `\n\n` when needed).
- Empty joined → same INVALID_PARAMS as today.
- Keep `parse_prompt(params)` as `materialize_prompt(params, None)` for backward-compatible call sites **or** replace all call sites.

In `handle_session_prompt`, after resolving `session_arc` / before turn, obtain `workspace_dir` from the session (`session.workspace_dir` / stored path string) and call:

```rust
let prompt = Self::materialize_prompt(params, Some(Path::new(&workspace_dir)))?;
```

instead of `parse_prompt` alone. Move the prompt materialization to **after** the session is found so `workspace_dir` is available (today `parse_prompt` runs before session lookup — reorder: validate sessionId → lock session → read workspace_dir → materialize_prompt).

Test cases (unit, sync):
1. blob-only array + temp workspace → marker in string + file exists.
2. bad base64 → INVALID_PARAMS.
3. `materialize_prompt` with blob but `workspace_dir: None` → INVALID_PARAMS mentioning workspace/blob.

- [ ] **Step 5: Implement + run tests**

```bash
cargo test -p zeroclaw-channels --features channel-acp-server parse_prompt -- --nocapture
cargo test -p zeroclaw-channels --features channel-acp-server materialize_prompt -- --nocapture
cargo test -p zeroclaw-channels --features channel-acp-server initialize_response_uses_acp_v1_shape -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/zeroclaw-channels/src/orchestrator/acp_server.rs
git commit -m "$(cat <<'EOF'
feat(acp): accept resource.blob in session/prompt and advertise embeddedContext

Persist blobs under the session workspace and surface Document/IMAGE markers.
EOF
)"
```

---

### Task 3: `deliver_file` tool

**Files:**
- Create: `crates/zeroclaw-runtime/src/tools/deliver_file.rs`
- Modify: `crates/zeroclaw-runtime/src/tools/mod.rs`

- [ ] **Step 1: Write tool unit tests** (in `deliver_file.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityPolicy;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn policy_for(root: &std::path::Path) -> Arc<SecurityPolicy> {
        // Mirror other tool tests: SecurityPolicy scoped to root.
        // Use the same constructor pattern as file_read tests in this crate.
        Arc::new(SecurityPolicy::from_workspace_dir(root)) // adjust to real API used in file_read tests
    }

    #[tokio::test]
    async fn delivers_json_with_path_and_mime() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a.pdf");
        std::fs::write(&file, b"%PDF-1.4").unwrap();
        let tool = DeliverFileTool::new(policy_for(dir.path()));
        let result = tool
            .execute(serde_json::json!({"path": "a.pdf", "mimeType": "application/pdf"}))
            .await
            .unwrap();
        assert!(result.success);
        let data = result.output.data().expect("structured data");
        assert_eq!(data["mimeType"], "application/pdf");
        assert!(data["path"].as_str().unwrap().contains("a.pdf"));
        assert!(result.output.as_str().contains("Delivered"));
    }

    #[tokio::test]
    async fn rejects_path_escape() {
        let dir = tempdir().unwrap();
        let tool = DeliverFileTool::new(policy_for(dir.path()));
        let result = tool
            .execute(serde_json::json!({"path": "../outside.txt"}))
            .await;
        assert!(result.is_err() || result.as_ref().ok().is_some_and(|r| !r.success));
    }
}
```

Inspect `file_read.rs` tests for the exact `SecurityPolicy` constructor and copy that pattern (do not invent `from_workspace_dir` if it does not exist).

- [ ] **Step 2: Run tests — expect FAIL (module missing)**

```bash
cargo test -p zeroclaw-runtime deliver_file -- --nocapture
```

- [ ] **Step 3: Implement `DeliverFileTool`**

Mirror `FileReadTool` structure:

```rust
pub struct DeliverFileTool {
    security: Arc<SecurityPolicy>,
}

impl DeliverFileTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self { Self { security } }
}

#[async_trait]
impl Tool for DeliverFileTool {
    fn name(&self) -> &str { "deliver_file" }
    fn description(&self) -> &str {
        "Deliver a file from the workspace to the ACP client as an embedded binary resource \
         (PDF, DOCX, images, etc.). Use when the user should download or preview the file. \
         Path must stay inside the workspace."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Workspace-relative or absolute path inside the workspace" },
                "mimeType": { "type": "string", "description": "Optional MIME type; guessed from extension if omitted" }
            },
            "required": ["path"]
        })
    }
    fn output_schema(&self) -> Option<serde_json::Value> {
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
        // resolve path like FileReadTool::resolve_candidate
        // security.check / allowed path
        // metadata + read; if len > 10MB bail
        // mime from args or guess
        // Ok(ToolResult::ok(ToolOutput::json_with_text(data, format!("Delivered {filename} ({bytes} bytes)"))))
    }
}
```

Register in `default_tools_with_runtime` (and any parallel registry builder that lists file tools) wrapped like other file tools (`PathGuardedTool` + `RateLimitedTool` if that is the local pattern for `file_read`).

Update `default_tools_names` test to `assert!(names.contains(&"deliver_file"));`.

Update `map_tool_kind` in `acp_server.rs` to map `"deliver_file" => "other"` (or `"read"`).

- [ ] **Step 4: Run runtime + channels compile**

```bash
cargo test -p zeroclaw-runtime deliver_file -- --nocapture
cargo test -p zeroclaw-runtime default_tools_names -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/zeroclaw-runtime/src/tools/deliver_file.rs crates/zeroclaw-runtime/src/tools/mod.rs crates/zeroclaw-channels/src/orchestrator/acp_server.rs
git commit -m "$(cat <<'EOF'
feat(tools): add deliver_file for ACP binary delivery

Workspace-jailed tool that returns path/mime metadata for ACP resource emission.
EOF
)"
```

---

### Task 4: ACP `tool_call_update` emits `resource`+`blob` for `deliver_file`

**Files:**
- Modify: `crates/zeroclaw-channels/src/orchestrator/acp_server.rs` (`notification_for_turn_event`)

- [ ] **Step 1: Write a unit test on notification content**

```rust
#[test]
fn deliver_file_tool_result_includes_resource_blob_not_in_raw_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.pdf");
    std::fs::write(&path, b"%PDF").unwrap();
    let output = serde_json::json!({
        "delivered": true,
        "path": path.to_string_lossy(),
        "filename": "x.pdf",
        "mimeType": "application/pdf",
        "bytes": 4
    })
    .to_string();
    // Prefer pretty JSON matching ToolOutput::json display, or accept both:
    let output = serde_json::to_string_pretty(&serde_json::json!({
        "delivered": true,
        "path": path.to_string_lossy(),
        "filename": "x.pdf",
        "mimeType": "application/pdf",
        "bytes": 4
    })).unwrap();

    let event = TurnEvent::ToolResult {
        id: "tc1".into(),
        name: "deliver_file".into(),
        output: output.clone(),
    };
    let n = notification_for_turn_event("s1", &event).unwrap();
    let update = &n.params["update"];
    assert_eq!(update["rawOutput"], output); // or assert rawOutput is the human summary only — see step 3
    let content = update["content"].as_array().unwrap();
    assert!(content.iter().any(|c| {
        c.pointer("/content/type").and_then(|v| v.as_str()) == Some("resource")
            && c.pointer("/content/resource/blob").and_then(|v| v.as_str()).is_some()
            && c.pointer("/content/resource/mimeType").and_then(|v| v.as_str()) == Some("application/pdf")
    }));
    let raw = update["rawOutput"].as_str().unwrap();
    assert!(!raw.contains("JVBE") && raw.len() < 10_000); // no giant base64 dump
}
```

Adjust `TurnEvent` import path to match the crate (`zeroclaw_api::agent::TurnEvent`).

- [ ] **Step 2: Run test — expect FAIL**

```bash
cargo test -p zeroclaw-channels --features channel-acp-server deliver_file_tool_result_includes_resource_blob -- --nocapture
```

- [ ] **Step 3: Implement special-case in `notification_for_turn_event`**

For `TurnEvent::ToolResult { id, name, output }` when `name == "deliver_file"`:
1. Try `serde_json::from_str::<Value>(output)`.
2. Read `path`, `mimeType`, `filename` fields.
3. `std::fs::read(path)`; on success base64-encode; build `uri` as `file://{path}` (normalize slashes) or `attachment://deliver/{filename}`.
4. Set:
   - `rawOutput` / `body` = short text: use `output`’s `Delivered …` if you switch tool to `json_with_text`, **or** format from JSON fields (`Delivered {filename} ({bytes} bytes)`). Prefer **human summary in rawOutput**, not full JSON, so clients/logs stay small.
5. `content`: array with text item + resource item:

```json
[
  { "type": "content", "content": { "type": "text", "text": "<summary>" } },
  { "type": "content", "content": {
      "type": "resource",
      "resource": { "uri": "...", "mimeType": "...", "blob": "<b64>" }
  }}
]
```

6. On parse/IO failure: fall back to today’s text-only content behavior.

Align Task 3 tool output with this: use `ToolOutput::json_with_text(data, summary)` so `TurnEvent` string is the **summary** only — then ACP must get path from… **problem**: TurnEvent only has the summary string.

**Required approach (pick one and implement consistently):**

**Chosen:** Put a single-line machine trailer in the summary that humans can ignore:

```text
Delivered x.pdf (4 bytes)
acp.deliver_file path=/abs/x.pdf mimeType=application/pdf
```

Parse that trailer in `notification_for_turn_event`. Keep `rawOutput` equal to the full summary (trailer included is OK; still tiny). Do **not** put file bytes in `rawOutput`.

Update Task 3 `execute` accordingly if not already.

- [ ] **Step 4: Tests PASS**

```bash
cargo test -p zeroclaw-channels --features channel-acp-server deliver_file_tool_result -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/zeroclaw-channels/src/orchestrator/acp_server.rs crates/zeroclaw-runtime/src/tools/deliver_file.rs
git commit -m "$(cat <<'EOF'
feat(acp): emit resource.blob on deliver_file tool results

ACP clients receive standard embedded resources without base64 in rawOutput.
EOF
)"
```

---

### Task 5: Docs + fork rebuild note

**Files:**
- Modify: `docs/book/src/channels/acp.md`
- Optional note in design spec only (already has roadmap)

- [ ] **Step 1: Update `acp.md`**

1. In the sample `initialize` response, set `"embeddedContext": true`.
2. Document that `session/prompt` array parts may include `resource.blob` (base64), persisted under `{workspaceDir}/uploads/`, surfaced as `[Document: …]` / `[IMAGE: …]`, 10 MB limit.
3. Document tool `deliver_file` and that `tool_call_update.content` may include a `resource` block.
4. Note `image`/`audio` capabilities remain false (roadmap P1/P2).

- [ ] **Step 2: Commit**

```bash
git add docs/book/src/channels/acp.md
git commit -m "$(cat <<'EOF'
docs(acp): document embeddedContext blobs and deliver_file

EOF
)"
```

- [ ] **Step 3: Remind human**

After merge to fork workflow: add `feat/acp-embedded-resource-blob` to `$Branches` in `dev-local/rebuild-main.ps1` (on `local/dev-tooling`), then rebuild `main`. Open upstream issue linking P0 + roadmap (do not file unless user asks).

---

### Task 6: Full verification

- [ ] **Step 1: Run focused suites**

```bash
cargo test -p zeroclaw-channels --features channel-acp-server acp_embedded -- --nocapture
cargo test -p zeroclaw-channels --features channel-acp-server materialize_prompt -- --nocapture
cargo test -p zeroclaw-channels --features channel-acp-server deliver_file -- --nocapture
cargo test -p zeroclaw-channels --features channel-acp-server initialize_response -- --nocapture
cargo test -p zeroclaw-runtime deliver_file -- --nocapture
cargo test -p zeroclaw-runtime default_tools_names -- --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Spec coverage check**

Confirm P0 items from the design spec are done: embeddedContext, inbound blob, deliver_file outbound resource, docs, tests. Image/audio wire caps still false. No Thunderbolt code.

---

## Self-review (plan vs spec)

| Spec requirement | Task |
|------------------|------|
| `embeddedContext: true` | Task 2 |
| Inbound blob → uploads + markers | Tasks 1–2 |
| 10 MB / bad base64 errors | Tasks 1–2 |
| `image/*` → `[IMAGE:]` without `image: true` | Task 1 |
| `deliver_file` jail + 10 MB | Task 3 |
| ACP content resource blob; no base64 in rawOutput | Task 4 |
| Docs | Task 5 |
| No TB / no file/attach / no audio | Honored (out of scope) |
