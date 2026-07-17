# Plan: MCP embedded resource blob intake

**Spec:** `docs/superpowers/specs/2026-07-17-mcp-embedded-resource-blob-intake-design.md`  
**Depends on:** P0 `feat/acp-embedded-resource-blob`

## Tasks

- [x] Move `materialize_resource_blob` to `zeroclaw-tools::embedded_resource`; ACP re-exports
- [x] Postprocess MCP tools/call results (`resource`+`blob` → uploads + markers)
- [x] Thread `SecurityPolicy` into `McpToolWrapper` / deferred activation (workspace at use time)
- [x] Unit tests (intake + materialize)
- [x] Book docs (MCP + ACP cross-link)
- [ ] Verify targeted cargo tests; open stacked PR
