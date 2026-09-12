# Config section-picker and wizard-preset strings. Loaded exactly like
# tools.ftl: this English file is compiled in as the base; a non-English
# locale overrides per key from <config_dir>/data/ftl/<locale>/sections.ftl.
# Keys mirror the stable identifiers the Rust builders use (memory backend
# key, risk/runtime preset_name, storage key).

# Memory backends (zeroclaw-memory selectable backends)
picker-memory-sqlite = SQLite with Vector Search (recommended) — fast, hybrid search, embeddings
picker-memory-lucid = Lucid Memory bridge — sync with local lucid-memory CLI, keep SQLite fallback
picker-memory-postgres = PostgreSQL — remote durable storage via [storage.model_provider.config]
picker-memory-markdown = Markdown Files — simple, human-readable, no dependencies
picker-memory-none = None — disable persistent memory

# Risk presets (Quickstart)
picker-risk-locked_down = Locked Down
picker-risk-locked_down-desc = Tightest defaults. Workspace-only filesystem access, approval required for medium and high risk, no shell environment passthrough.
picker-risk-balanced = Balanced
picker-risk-balanced-desc = Trusted daily driver for a personal dev box. Supervised, workspace-scoped with sensitive paths blocked and the sandbox on. Any command runs without an allowlist, but high-risk commands stay blocked unless explicitly allowlisted. Recommended for most users.
picker-risk-yolo = YOLO
picker-risk-yolo-desc = Full autonomy. No approval gates, no command denylist, no workspace scoping. Only pick this if you know what you're doing on a machine you don't mind breaking.

# Runtime presets (Quickstart)
picker-runtime-tight = Tight
picker-runtime-tight-desc = Small budgets and short timeouts. Good for cheap models, metered API keys, or tight feedback loops where you want the agent to stop and ask early rather than burn budget.
picker-runtime-local_small = Local Small
picker-runtime-local_small-desc = Compact no-text-fallback profile for smaller local models. Keeps context and tool results small, disables parallel tool fan-out, and requires native or structured tool calls.
picker-runtime-balanced = Balanced
picker-runtime-balanced-desc = Middle-of-the-road operational defaults. Suits most users most of the time.
picker-runtime-unbounded = Unbounded
picker-runtime-unbounded-desc = Wide-open budgets and long timeouts. Pick this when you're actively driving the agent through a hard task and don't want it to throttle.

# Storage backends
picker-storage-sqlite-desc = Safe default for single-node installs: file-based, zero-config, no external service.
picker-storage-postgres-desc = Shared or multi-instance deployments that need durable server-backed storage.
picker-storage-qdrant-desc = Vector database backend for semantic search when you already run Qdrant.
picker-storage-markdown-desc = Human-readable files with simple local storage and no database service.
picker-storage-lucid-desc = Bridge to local lucid-memory CLI while keeping SQLite-style local operation.

# Providers / tunnel
picker-provider-local-desc = Local — no API key required
picker-tunnel-none-desc = Localhost only — no public tunnel.
