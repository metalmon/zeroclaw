# MCP embedded resource blob intake

**Date:** 2026-07-17  
**Branch:** `feat/mcp-embedded-resource-blob-intake` (stacks on `feat/acp-embedded-resource-blob`)  
**Status:** Approved direction; implementation on this branch  
**Depends on:** P0 ACP embedded resource blob (`materialize_resource_blob`, inbound + `deliver_file`)

## Problem

MCP `tools/call` results that include embedded binary content
(`content[]` item with `type: "resource"` and nested `blob` base64) are today
pretty-printed via `serde_json::to_string_pretty` into the model-facing tool
output. That dumps megabytes of base64 into context, burns tokens, and makes
the bytes unusable as workspace files for follow-up tools (e.g. `deliver_file`).

## Solution

After a successful MCP tools/call, **before** serializing the full result for the
model:

1. Detect content items by **shape**: `type == "resource"` and `resource.blob`
   present (not by tool name / vendor).
2. Call the shared `materialize_resource_blob` helper → write under
   `{workspace}/uploads/<sha16>.<ext>`.
3. Replace model-facing output with provenance text (non-blob content parts) plus
   `[Document: …]` / `[IMAGE:…]` markers.
4. Invalid base64 or oversize blobs fail cleanly as a non-fatal tool error.

## Shared helper

- Canonical implementation: `zeroclaw_tools::embedded_resource`
- ACP re-exports via `acp_embedded` (no `zeroclaw-tools` → `zeroclaw-channels` dep)
- Workspace path resolved from `SecurityPolicy` at execute time (handle on
  `McpToolWrapper` / deferred set — not a cached `PathBuf` copy)

## Non-goals

- No new model-facing tool
- No auto-deliver to ACP (agent still calls `deliver_file` explicitly)
- No Glossa / vendor-specific tool-name hardcoding
- No change to MCP `resources/read` pinning path beyond existing blob redaction

## Testing

- Unit: blob → file + marker; no base64 in output; bad base64 / oversized fail
- ACP materialize re-export / existing ACP + `deliver_file` tests still pass
