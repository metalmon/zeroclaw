# ACP embedded resource blob (inbound + outbound)

**Date:** 2026-07-17  
**Branch:** `feat/acp-embedded-resource-blob` (from `origin/master`)  
**Status:** Design approved; awaiting implementation plan

## Problem

ZeroClaw’s ACP server advertises `embeddedContext: false` and `parse_prompt` only inlines `resource.text`. Binary ACP `resource.blob` payloads are ignored. Tool results are always emitted as text, so an agent cannot deliver a PDF/DOCX (or other bytes) back to an ACP client for preview/download.

External clients (e.g. Thunderbolt) need the **standard ACP** ContentBlock shape:

```json
{
  "type": "resource",
  "resource": {
    "uri": "file:///…/report.pdf",
    "mimeType": "application/pdf",
    "blob": "<base64>"
  }
}
```

This must not be confused with the gateway RPC `file/attach` wire (#6819), which is a separate product-chat/Zerocode contract. Maintainers closed RFC #8798 (`wontfix`) preferring ACP as a thin IDE/interop contract, with room for **narrow** ACP extensions when external clients need them.

## Goals

1. **Inbound:** Accept ACP `resource` blocks with `blob` in `session/prompt`; persist bytes under the session workspace; expose a path/marker to the model.
2. **Outbound:** Explicit tool `deliver_file` so the agent can push a workspace file to the client as ACP `resource`+`blob` in `tool_call_update.content`.
3. Advertise `promptCapabilities.embeddedContext: true`.
4. Document behavior in `docs/book/src/channels/acp.md`.
5. Stay on standard ACP ContentBlocks; no Haystack; no Thunderbolt changes in this PR.

## Non-goals

- Thunderbolt UI / `putAttachment` / sideview (follow-up client PR).
- Calling or exposing RPC `file/attach` from ACP.
- Auto-wrapping arbitrary tool outputs as blobs.
- Enabling `image` / `audio` prompt capabilities.
- Chunked upload, `file/download` RPC, or merging `/ws/chat` onto ACP.

## Architecture

```
Client                         ZeroClaw ACP
──────                         ────────────
session/prompt
  resource{uri,mime,blob}  →   parse_prompt
                               shared helper (decode, 10MB, sha dedup)
                               write {workspaceDir}/uploads/<sha16>.<ext>
                               prompt += [Document|IMAGE] path marker
                               agent turn
deliver_file(path)         ←   tool (workspace-jailed)
tool_call_update           ←   text summary + content resource{blob}
```

**Shared helper** reuses *ideas* from `process_file_entry` (size limits, sha naming, uploads dir) but must **not** depend on RPC `SessionStore` or the `file/attach` method. Prefer a small store-agnostic function callable from `acp_server` (and optionally later from RPC).

## Inbound

### Capability

`initialize` → `agentCapabilities.promptCapabilities.embeddedContext: true`  
(`image` / `audio` remain `false`).

### Prompt intake (`session/prompt`)

Today `parse_prompt` is a pure string join and has no `workspaceDir`. Blob persistence therefore belongs in **`handle_session_prompt`** (or a helper it calls) after the session’s `workspaceDir` is known—not inside a workspace-blind string parser.

1. Keep joining `text` parts and `resource.text` as today.
2. When `resource.blob` is present:
   - Base64-decode; reject invalid base64 with `INVALID_PARAMS`.
   - Enforce **10 MB** decoded size per file (`INVALID_PARAMS` if larger).
   - Filename from `uri` basename, else `upload.bin`.
   - Optional `mimeType`; guess from extension when absent.
   - Write to `{session.workspaceDir}/uploads/<sha256[:16]>.<ext>` (create dir as needed). If that path already exists with the same hash prefix, reuse it (simple fs dedup; no RPC upload index for v1).
   - Append to the prompt string:
     - `image/*` → `[IMAGE:<abs-path>]`
     - otherwise → `[Document: <name>] <abs-path>`
3. Blob-only parts (no sibling text) are valid.
4. If the joined prompt is empty → existing `INVALID_PARAMS`.

Session `workspaceDir` is the ACP session cwd already established at `session/new`.

## Outbound

### Tool `deliver_file`

**Input:**

| Field | Required | Notes |
|-------|----------|--------|
| `path` | yes | Relative to workspace or absolute; must stay inside SecurityPolicy jail |
| `mimeType` | no | Guessed from extension if omitted |

**Behavior:**

1. Resolve and jail-check like `file_read`.
2. Read file; reject if missing or **> 10 MB**.
3. Model-facing tool result: short text, e.g. `Delivered report.pdf (12345 bytes)`.
4. On the ACP notification path (`notification_for_turn_event` for `ToolResult` where `name == "deliver_file"`):
   - Emit `content` with:
     - a text content item (summary), and
     - a nested content item whose inner block is `type: "resource"` with `uri`, `mimeType`, `blob`.
   - Keep `rawOutput` as the short text summary only (do **not** put base64 in `rawOutput`).

If the tool fails jail/IO/size checks, emit a normal tool error; no resource block.

Register the tool in the default/runtime tool set so ACP agents can call it. Non-ACP channels may still invoke it; they only see the text summary unless a future channel maps the same semantics.

## Error table

| Case | Response |
|------|----------|
| Invalid base64 | `INVALID_PARAMS` |
| Inbound/outbound file > 10 MB | `INVALID_PARAMS` / tool error |
| `deliver_file` path escapes workspace | tool error |
| File not found | tool error |
| Empty prompt after parse | `INVALID_PARAMS` (existing) |

## Testing

- `parse_prompt`: text-only; `resource.text`; `resource.blob` writes file + marker; oversized; bad base64; image mime → `[IMAGE:…]`.
- `initialize`: `embeddedContext == true`.
- `deliver_file` + ACP notification: `content` includes `resource.blob`; `rawOutput` has no giant base64.
- Jail: path escape rejected.

## Docs / upstream

- Update `docs/book/src/channels/acp.md` (capability, prompt example with blob, `deliver_file`, limits).
- Open or link a focused upstream issue: “ACP embedded resource blob + deliver_file” (narrow extension; cite #8798 closure guidance).

## Follow-ups (out of this PR)

1. Thunderbolt: ingest outbound `resource`+`blob` → local attachment → PDF/DOCX sideview.
2. Optional: extract fully shared helper used by both RPC attach and ACP.
3. Optional: Glossa/MCP path that materializes KB originals for `deliver_file`.

## Decision log

| Topic | Choice |
|-------|--------|
| Scope | Inbound + outbound in one Zeroclaw PR |
| Inbound storage | Shared helper → `{workspaceDir}/uploads/…`, not RPC `file/attach` |
| Outbound trigger | Explicit `deliver_file` only |
| Client | Zeroclaw only; Thunderbolt later |
| Approach | Minimal ACP surface (standard ContentBlocks) |
