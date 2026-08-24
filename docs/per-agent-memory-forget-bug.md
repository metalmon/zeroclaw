# BUG: per-agent memory forget is blocked (agent_id not stamped on stored rows)

Status: OPEN, not yet fixed. Recorded 2026-08-10 on branch
`fix/per-agent-memory-autosave-clean`. This branch fixed the per-agent memory
**autosave routing** (webhook + heartbeat write to the addressed agent's
backend); this note captures a **separate, still-open** per-agent memory defect
found afterward. Do not lose it when picking this branch back up.

## Symptom (from a live agent)

- `memory_store` works: a record is saved.
- `memory_forget` does not work: it reports success-ish but the row is never
  removed.
- Even a freshly stored record immediately appears "nameless" (no owner), and
  `AgentScopedMemory` then refuses to delete it.

In short: an agent can put things into memory but cannot take them out.

## What is verified (wrapper flow)

`crates/zeroclaw-memory/src/agent_scoped.rs` (`AgentScopedMemory`) is correct on
its own side:

- `store` calls `store_with_options_and_agent(.., Some(&self.agent_id), ..)`, so
  the wrapper DOES pass the bound `agent_id` down to the inner backend.
- `forget(key)` delegates to `inner.forget_for_agent(key, &self.agent_id)`, so
  deletion requires the row to carry a matching `agent_id`.
- `list` keeps only entries where `e.agent_id.is_some_and(|aid| allowed.contains(aid))`,
  so a row whose `agent_id` is `None` is invisible to the owner and cannot be
  targeted.

So the wrapper passes ownership in on write and requires ownership on delete.

## Likely root cause (hypothesis, verify before fixing)

The concrete inner backend does not persist the `agent_id` it is handed at
store time (writes the row with `agent_id = NULL`). Every stored row is then
un-owned, so `forget_for_agent` matches nothing and `list` filters it out. This
matches the "record is nameless the moment it is written" observation.

The agent named `AgentScopedMemory` specifically (the SQL/Qdrant single-shared
backend + `agent_id` column path), not `AgentScopedMarkdownMemory`, so the
inner backend in play is the SQL/Qdrant one, most likely SQLite.

## Candidate files to inspect

- `crates/zeroclaw-memory/src/sqlite.rs` (most likely): its
  `store_with_options_and_agent` / `store_with_agent` write path and whether the
  `agent_id` column is actually bound in the INSERT, plus the WHERE clause in
  `forget_for_agent`.
- `crates/zeroclaw-memory/src/postgres.rs`: same checks if the deployed backend
  is Postgres.
- `crates/zeroclaw-memory/src/qdrant.rs`: payload agent_id on upsert vs delete
  filter.
- `crates/zeroclaw-memory/src/agent_scoped.rs`: confirm no path stores without
  the agent_id (e.g. a plain `store` on the inner that bypasses
  `store_*_and_agent`).

## Reproduce

1. Configure a named agent with a SQL/Qdrant memory backend (per-agent scoping
   on).
2. As that agent, `memory_store` a key.
3. `memory_list` and confirm whether the entry shows an owner / agent_id.
4. `memory_forget` the same key and confirm it is not removed.
5. Inspect the stored row directly (e.g. the SQLite `agent_id` column) to see
   whether it is NULL.

## Fix direction (later)

Ensure the inner backend persists the passed `agent_id` on every store path, so
`forget_for_agent` and the `list` ownership filter both match. Add a regression:
store as agent A, assert the row carries `agent_id = A`, then assert
`forget` removes it and `list` no longer returns it.
