# `deliver_file` Returns `uri` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make successful `deliver_file` model-facing results include a `uri` field that is byte-identical to the ACP `resource.uri` already sent to the client.

**Architecture:** Compute `attachment://deliver/<basename>` once via a single helper in `deliver_file`, put that string in structured JSON + summary text, and have the ACP notification path reuse the same helper (or the `uri=` line from the tool output) when building `resource.uri` — never a second ad-hoc formatter. No ACP protocol extension (`filename` / required `_meta` stay rejected). MCP blob intake stays out of this PR.

**Tech Stack:** Rust, `zeroclaw-runtime` tools (`DeliverFileTool`), `zeroclaw-channels` ACP server (`notification_for_turn_event`), `serde_json`, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-07-18-deliver-file-return-uri-design.md`  
**Branch:** `feat/acp-embedded-resource-blob` (do **not** mix into `feat/mcp-embedded-resource-blob-intake`)  
**Sequencing:** Finish this plan before Thunderbolt citations/widgets plan.

---

## File map

| File | Responsibility |
|------|----------------|
| `crates/zeroclaw-runtime/src/tools/deliver_file.rs` | Source of truth helper `attachment_deliver_uri`; add `uri` to JSON + summary + `output_schema`; tool description note |
| `crates/zeroclaw-runtime/src/tools/mod.rs` | Re-export `attachment_deliver_uri` for ACP |
| `crates/zeroclaw-channels/src/orchestrator/acp_server.rs` | Build `resource.uri` from the same helper / parsed `uri=` line; equality + no-`filename` tests |
| `docs/book/src/channels/acp.md` | Document that model result includes `uri` identical to wire |

---

### Task 1: Failing tests — `uri` in `deliver_file` result

**Files:**
- Modify: `crates/zeroclaw-runtime/src/tools/deliver_file.rs` (tests module only in this task)

- [ ] **Step 1: Write the failing tests**

In `crates/zeroclaw-runtime/src/tools/deliver_file.rs`, inside `mod tests`, add:

```rust
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
```

Also update the existing `delivers_json_with_path_and_mime` test to assert `uri` once implementation exists (add the assertion in Task 2 after the helper exists — for now only the new tests above are required to fail).

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p zeroclaw-runtime --lib tools::deliver_file::tests::success_json_includes_attachment_deliver_uri tools::deliver_file::tests::failure_omits_success_uri tools::deliver_file::tests::attachment_deliver_uri_helper_formats_basename -- --nocapture
```

Expected: FAIL — `attachment_deliver_uri` not found and/or `data["uri"]` is null / missing.

- [ ] **Step 3: Commit the failing tests**

```bash
git add crates/zeroclaw-runtime/src/tools/deliver_file.rs
git commit -m "$(cat <<'EOF'
test(deliver_file): require uri in success result

EOF
)"
```

---

### Task 2: Implement `uri` in `deliver_file` (+ helper)

**Files:**
- Modify: `crates/zeroclaw-runtime/src/tools/deliver_file.rs`
- Modify: `crates/zeroclaw-runtime/src/tools/mod.rs`

- [ ] **Step 1: Add the helper and wire it into success output**

Near the top of `deliver_file.rs` (after `MAX_DELIVER_FILE_BYTES`), add:

```rust
/// ACP / model citation URI for an outbound delivered file.
///
/// Source of truth for the `attachment://deliver/<basename>` string — ACP must
/// reuse this helper (or the `uri=` line emitted below), not a second formatter.
pub fn attachment_deliver_uri(basename: &str) -> String {
    format!("attachment://deliver/{basename}")
}
```

Update `output_schema` properties to include `uri`:

```rust
    fn output_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "delivered": { "type": "boolean" },
                "uri": { "type": "string" },
                "path": { "type": "string" },
                "filename": { "type": "string" },
                "mimeType": { "type": "string" },
                "bytes": { "type": "integer" }
            }
        }))
    }
```

In `execute`, after computing `filename` / `mime_type` / `abs_path` / `bytes`, compute uri once and include it:

```rust
        let uri = attachment_deliver_uri(&filename);

        let summary = format!(
            "Delivered {filename} ({bytes} bytes)\nuri={uri}\nacp.deliver_file path={abs_path} mimeType={mime_type}"
        );
        let data = json!({
            "delivered": true,
            "uri": uri,
            "path": abs_path,
            "filename": filename,
            "mimeType": mime_type,
            "bytes": bytes,
        });
```

Update tool `description` so the agent knows to copy `uri` (skill/prompt note for ZC):

```rust
    fn description(&self) -> &str {
        "Deliver a file from the workspace to the ACP client as an embedded binary resource \
         (PDF, DOCX, images, etc.). Use when the user should download or preview the file. \
         Path must stay inside the workspace. On success the result includes `uri` \
         (`attachment://deliver/<basename>`) — cite that exact uri in widgets/`[N]`; \
         do not invent prefixes. Pretty display names come from `[Document: …]` markers, \
         not from inventing ACP filename fields."
    }
```

In `crates/zeroclaw-runtime/src/tools/mod.rs`, change the re-export to:

```rust
pub use deliver_file::{attachment_deliver_uri, DeliverFileTool};
```

Also strengthen the existing success test:

```rust
        assert_eq!(data["uri"], "attachment://deliver/a.pdf");
        assert!(text.contains("uri=attachment://deliver/a.pdf"));
```

- [ ] **Step 2: Run tests to verify they pass**

Run:

```bash
cargo test -p zeroclaw-runtime --lib tools::deliver_file -- --nocapture
```

Expected: PASS (all `deliver_file` tests).

- [ ] **Step 3: Commit**

```bash
git add crates/zeroclaw-runtime/src/tools/deliver_file.rs crates/zeroclaw-runtime/src/tools/mod.rs
git commit -m "$(cat <<'EOF'
feat(deliver_file): return attachment://deliver uri in tool result

EOF
)"
```

---

### Task 3: Failing ACP tests — model uri === resource.uri, no filename

**Files:**
- Modify: `crates/zeroclaw-channels/src/orchestrator/acp_server.rs` (tests module)

- [ ] **Step 1: Write the failing tests**

In the existing `#[cfg(test)]` module of `acp_server.rs`, next to `deliver_file_tool_result_includes_resource_blob_not_in_raw_output`, add:

```rust
    #[test]
    fn deliver_file_resource_uri_matches_summary_uri_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a1b2c3d4e5f6.pdf");
        std::fs::write(&path, b"%PDF").unwrap();
        let abs = path.to_string_lossy();
        let uri = "attachment://deliver/a1b2c3d4e5f6.pdf";
        let output = format!(
            "Delivered a1b2c3d4e5f6.pdf (4 bytes)\nuri={uri}\nacp.deliver_file path={abs} mimeType=application/pdf"
        );

        let event = TurnEvent::ToolResult {
            id: "tc1".into(),
            name: "deliver_file".into(),
            output: output.clone(),
        };
        let n = notification_for_turn_event("s1", &event).unwrap();
        let update = &n.params["update"];
        let content = update["content"].as_array().unwrap();
        let resource_uri = content
            .iter()
            .find_map(|c| c.pointer("/content/resource/uri").and_then(|v| v.as_str()))
            .expect("resource uri");
        assert_eq!(resource_uri, uri);
        // No ACP protocol extension for pretty names:
        assert!(
            content
                .iter()
                .filter_map(|c| c.pointer("/content/resource"))
                .all(|r| r.get("filename").is_none()),
            "resource must not carry filename"
        );
        let raw = update["rawOutput"].as_str().unwrap();
        assert!(!raw.contains("JVBE") && raw.len() < 10_000);
    }

    #[test]
    fn deliver_file_resource_uri_uses_shared_helper_when_uri_line_absent() {
        // Backward-compat / trailer-only path: still must match attachment_deliver_uri(basename).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.pdf");
        std::fs::write(&path, b"%PDF").unwrap();
        let abs = path.to_string_lossy();
        let output = format!(
            "Delivered x.pdf (4 bytes)\nacp.deliver_file path={abs} mimeType=application/pdf"
        );
        let expected = zeroclaw_runtime::tools::attachment_deliver_uri("x.pdf");

        let event = TurnEvent::ToolResult {
            id: "tc1".into(),
            name: "deliver_file".into(),
            output,
        };
        let n = notification_for_turn_event("s1", &event).unwrap();
        let uri = n.params["update"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|c| c.pointer("/content/resource/uri").and_then(|v| v.as_str()))
            .unwrap();
        assert_eq!(uri, expected);
    }
```

- [ ] **Step 2: Run tests to verify the first new test fails (or is red on uri mismatch if current code already matches basename)**

Run:

```bash
cargo test -p zeroclaw-channels --lib orchestrator::acp_server::tests::deliver_file_resource_uri_matches_summary_uri_line orchestrator::acp_server::tests::deliver_file_resource_uri_uses_shared_helper_when_uri_line_absent -- --nocapture
```

Expected: FAIL compile (`attachment_deliver_uri` not re-exported yet — Task 2 should have fixed that) **or** FAIL assertion if `deliver_file_tool_result_content` still uses a local `format!` that drifts / ignores the `uri=` line. If both already pass because basename formatting happens to match, still proceed to Task 4 to delete the local formatter and prefer the `uri=` line (source-of-truth reuse).

- [ ] **Step 3: Commit failing tests**

```bash
git add crates/zeroclaw-channels/src/orchestrator/acp_server.rs
git commit -m "$(cat <<'EOF'
test(acp): require deliver_file resource.uri equals tool uri

EOF
)"
```

---

### Task 4: ACP path reuses the same uri string

**Files:**
- Modify: `crates/zeroclaw-channels/src/orchestrator/acp_server.rs:1927-1986`

- [ ] **Step 1: Parse optional `uri=` line; fall back to shared helper**

Replace the uri construction inside `deliver_file_tool_result_content` so it does **not** use a second ad-hoc `format!("attachment://deliver/…")`.

Add helpers above `deliver_file_tool_result_content`:

```rust
fn parse_deliver_file_uri_line(output: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        let Some(uri) = line.strip_prefix("uri=") else {
            continue;
        };
        let uri = uri.trim();
        if uri.starts_with("attachment://deliver/") {
            return Some(uri.to_string());
        }
    }
    None
}
```

Change `deliver_file_tool_result_content` body to:

```rust
fn deliver_file_tool_result_content(name: &str, output: &str) -> Option<Value> {
    if name != "deliver_file" {
        return None;
    }
    let (path, mime_type) = parse_deliver_file_trailer(output)?;
    let bytes = std::fs::read(&path).ok()?;
    let blob = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let filename = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    // Prefer the uri emitted by deliver_file (identical string); else shared helper.
    let uri = parse_deliver_file_uri_line(output)
        .unwrap_or_else(|| zeroclaw_runtime::tools::attachment_deliver_uri(filename));
    Some(serde_json::json!([
        {
            "type": "content",
            "content": {
                "type": "text",
                "text": output
            }
        },
        {
            "type": "content",
            "content": {
                "type": "resource",
                "resource": {
                    "uri": uri,
                    "mimeType": mime_type,
                    "blob": blob
                }
            }
        }
    ]))
}
```

Do **not** add `filename` (or any pretty-name field) to the `resource` object.

- [ ] **Step 2: Run ACP deliver_file tests**

Run:

```bash
cargo test -p zeroclaw-channels --lib orchestrator::acp_server::tests::deliver_file -- --nocapture
```

Expected: PASS — including blob-not-in-rawOutput regression, uri equality, no `filename`.

- [ ] **Step 3: Commit**

```bash
git add crates/zeroclaw-channels/src/orchestrator/acp_server.rs
git commit -m "$(cat <<'EOF'
fix(acp): reuse deliver_file uri for resource.uri

EOF
)"
```

---

### Task 5: Docs — contract note for TB / agents

**Files:**
- Modify: `docs/book/src/channels/acp.md` (section `#### Delivering files to the client (\`deliver_file\`)`)

- [ ] **Step 1: Document model-facing `uri`**

After the existing JSON example for ACP `resource`, add a short paragraph + example:

```markdown
The tool's model-facing result also includes the same `uri` string (structured JSON field `uri`, and a `uri=…` line in the text summary). Clients such as Thunderbolt materialize the outbound blob and build a citation ref-map keyed by that uri. Agents must copy the returned `uri` into `<widget:document-result fileId="…">` / `[N]` citations and must not invent prefixes. Pretty display names come from `[Document: …]` markers in the prompt — there is **no** `filename` field on the ACP `resource` object.
```

- [ ] **Step 2: Sanity-check markdown locally (optional lightweight)**

Open the section and confirm the example uri still reads `attachment://deliver/…` and no `filename` appears on `resource`.

- [ ] **Step 3: Commit**

```bash
git add docs/book/src/channels/acp.md
git commit -m "$(cat <<'EOF'
docs(acp): note deliver_file uri in model result for citations

EOF
)"
```

---

### Task 6: Final verification (this PR slice only)

- [ ] **Step 1: Run focused + related tests**

```bash
cargo test -p zeroclaw-runtime --lib tools::deliver_file -- --nocapture
cargo test -p zeroclaw-channels --lib orchestrator::acp_server::tests::deliver_file -- --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Confirm out-of-scope was not touched**

```bash
git diff origin/master...HEAD --stat
```

Expected: **no** MCP intake files / `feat/mcp-embedded-resource-blob-intake` scope. Only deliver_file uri + ACP reuse + docs/tests on this branch.

- [ ] **Step 3: No extra commit unless verification required fixes** — if a fix is needed, commit it with a conventional message, then stop.

---

## Spec coverage checklist (self-review)

| Spec requirement | Task |
|------------------|------|
| Success JSON contains `uri` | Task 1–2 |
| `uri` === ACP `resource.uri` | Task 3–4 |
| Single source of truth for uri string | Task 2 helper + Task 4 parse/`attachment_deliver_uri` |
| Error paths: no success-uri / no resource | Task 1 failure test; existing ACP trailer failure → no resource |
| No `filename` on ACP resource | Task 3–4 |
| No giant base64 in rawOutput | Task 3 asserts + existing regression test |
| Skill/prompt note (copy uri; pretty name from Document marker) | Task 2 tool description + Task 5 docs |
| No MCP intake mixed in | Task 6 check |
| TB ref-map / widgets | **Out of scope** — TB plan |

## Placeholder scan

No TBD/TODO steps. All code blocks are concrete.

## Type / name consistency

- Helper name: `attachment_deliver_uri` everywhere.
- URI scheme: `attachment://deliver/<basename>` only.
- JSON field: `uri` (not `fileUri` / `resourceUri`).
