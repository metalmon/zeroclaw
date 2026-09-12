# English tool descriptions (default locale, embedded at compile time)
#
# Keys follow the pattern: tool-{name-with-hyphens}
# e.g. "file_read" → "tool-file-read", "web_search_tool" → "tool-web-search-tool"
#
# Literal { and } in values must be escaped as {"{"}  and  {"}"} respectively.

tool-backup = Create, list, verify, and restore workspace backups

tool-browser = Web/browser automation with pluggable backends (agent-browser, rust-native, computer_use). Supports DOM actions plus optional OS-level actions (mouse_move, mouse_click, mouse_drag, key_type, key_press, screen_capture) through a computer-use sidecar. Use 'snapshot' to map interactive elements to refs (@e1, @e2). Enforces browser.allowed_domains for open actions.

tool-browser-delegate = Delegate browser-based tasks to a browser-capable CLI for interacting with web applications like Teams, Outlook, Jira, Confluence

tool-browser-open = Open an approved HTTPS URL in the system browser. Security constraints: allowlist-only domains, no local/private hosts, no scraping.

tool-channel-room = Create rooms and invite users through an active channel. Provide a channel key such as 'matrix.default', action 'create_room' or 'invite_user', and the action-specific room fields.
tool-channel-room-param-action = Room-management action to perform.
tool-channel-room-param-channel = Active channel key such as 'matrix.default'.
tool-channel-room-param-name = Optional room name for create_room.
tool-channel-room-param-topic = Optional room topic for create_room.
tool-channel-room-param-invites = Optional user IDs to invite while creating the room.
tool-channel-room-param-visibility = Optional room visibility for create_room.
tool-channel-room-param-encryption = Whether to request room encryption during create_room.
tool-channel-room-param-room-id = Existing room ID for invite_user.
tool-channel-room-param-user-id = User ID to invite for invite_user.
tool-channel-room-error-security = Action blocked: { $err }
tool-channel-room-error-invalid-action = Invalid action '{ $action }': must be 'create_room' or 'invite_user'.
tool-channel-room-error-not-initialized = No channels available yet (channels not initialized).
tool-channel-room-error-channel-not-found = Channel '{ $channel }' not found. Available channels: { $available }
tool-channel-room-error-create-failed = Failed to create room: { $err }
tool-channel-room-error-invite-failed = Failed to invite user: { $err }
tool-channel-room-error-invites-array = 'invites' must be an array of strings.
tool-channel-room-error-invites-item = 'invites' must be an array of non-empty strings.
tool-channel-room-error-invalid-visibility = Invalid room visibility: { $err }
tool-channel-room-error-missing-param = Missing '{ $param }' parameter.
tool-channel-room-error-string-param = '{ $param }' must be a string.
tool-channel-room-error-bool-param = '{ $param }' must be a boolean.

tool-cloud-ops = Cloud transformation advisory tool. Analyzes IaC plans, assesses migration paths, reviews costs, and checks architecture against Well-Architected Framework pillars. Read-only: does not create or modify cloud resources.

tool-cloud-patterns = Cloud pattern library. Given a workload description, suggests applicable cloud-native architectural patterns (containerization, serverless, database modernization, etc.).

tool-composio = Execute actions on 1000+ apps via Composio (Gmail, Notion, GitHub, Slack, etc.). Use action='list' to see available actions (includes parameter names). action='execute' with action_name/tool_slug and params to run an action. If you are unsure of the exact params, pass 'text' instead with a natural-language description of what you want (Composio will resolve the correct parameters via NLP). action='list_accounts' or action='connected_accounts' to list OAuth-connected accounts. action='connect' with app/auth_config_id to get OAuth URL. connected_account_id is auto-resolved when omitted.

tool-content-search = Search file contents by regex pattern within the workspace. Supports ripgrep (rg) with grep or internal fallback. Output modes: 'content' (matching lines with context), 'files_with_matches' (file paths only), 'count' (match counts per file). Example: pattern='fn main', include='*.rs', output_mode='content'.

tool-cron-add = Create a scheduled cron job (shell or agent) with cron/at/every schedules. Use job_type='agent' with a prompt to run the AI agent on schedule. To deliver output to a channel (Discord, Telegram, Slack, Mattermost, Matrix), set delivery={"{"}"mode":"announce","channel":"discord","to":"<channel_id_or_chat_id>"{"}"}. This is the preferred tool for sending scheduled/delayed messages to users via channels.

tool-cron-list = List all scheduled cron jobs

tool-cron-remove = Remove a cron job by id

tool-cron-run = Force-run a cron job immediately and record run history

tool-cron-runs = List recent run history for a cron job

tool-cron-update = Patch an existing cron job (schedule, command, prompt, enabled, delivery, model, etc.)

tool-data-management = Workspace data retention, purge, and storage statistics

tool-delegate = Delegate a subtask to a specialized agent. Use when: a task benefits from a different model (e.g. fast summarization, deep reasoning, code generation). The sub-agent runs a single prompt by default; with agentic=true it can iterate with a filtered tool-call loop.

tool-file-edit = Edit a file by replacing an exact string match with new content

tool-file-download = Download a file from the configured remote endpoint and write it to the agent's workspace. Supply the identifier of the document to fetch and a workspace-relative destination path; the endpoint URL is fixed by host config and is never model-controlled. Bytes are streamed straight to disk and are not loaded into model context. Returns the HTTP status, the number of bytes written, and the destination path.
tool-file-download-param-document-id = Identifier of the document to fetch from the configured endpoint.
tool-file-download-param-dest-path = Workspace-relative path to write the file to. The parent directory must already exist.
tool-file-download-error-disabled = file_download is disabled: [file_download].url is not configured
tool-file-download-error-read-only = Action blocked: autonomy is read-only
tool-file-download-error-rate-limited-hour = Rate limit exceeded: too many actions in the last hour
tool-file-download-error-rate-limited-budget = Rate limit exceeded: action budget exhausted
tool-file-download-error-missing-document-id = Missing 'document_id' parameter
tool-file-download-error-missing-dest-path = Missing 'dest_path' parameter
tool-file-download-error-invalid-file-name = Invalid dest_path '{ $dest_path }': must end in a concrete file name
tool-file-download-error-no-parent = Invalid dest_path '{ $dest_path }': has no parent directory
tool-file-download-error-resolve-dir = Cannot resolve destination directory for '{ $dest_path }': { $err }
tool-file-download-error-client-build = Failed to build download client: { $err }
tool-file-download-error-request = Download request failed: { $err }
tool-file-download-error-status = Download endpoint returned status { $status }
tool-file-download-error-too-large-reported = Download too large: endpoint reports { $len } bytes (limit: { $limit } bytes)
tool-file-download-error-too-large-stream = Download too large: exceeded limit of { $limit } bytes
tool-file-download-error-temp-create = Failed to create temporary download file: { $err }
tool-file-download-error-read-body = Failed while reading response body: { $err }
tool-file-download-error-write-body = Failed while writing downloaded bytes: { $err }
tool-file-download-error-flush = Failed to flush downloaded file: { $err }
tool-file-download-error-move = Failed to move downloaded file into place: { $err }
tool-file-download-success = Downloaded { $written } bytes to { $dest_path } ({ $status })

tool-file-read = Read file contents with line numbers. Supports partial reading via offset and limit. Binary and image files are rejected (use the image_info tool for images). Set encoding="base64" to return raw bytes base64-encoded (for binary files such as .pdf/.xlsx/.docx); offset/limit are ignored in that mode.

tool-file-write = Write contents to a file in the workspace

tool-git-operations = Perform structured Git operations (status, diff, log, branch, commit, add, checkout, stash). Provides parsed JSON output and integrates with security policy for autonomy controls.
tool-git-operations-error-not-in-repo = Not in a Git repository at '{ $path }'. Choose a path inside a Git worktree, pass 'path' for a repository subdirectory, or initialize a repository before running git_operations.

tool-git-forge-error-requires-field = { $resource }.{ $action } requires '{ $field }'.
tool-git-forge-error-requires-number = { $resource }.{ $action } requires 'number'.
tool-git-forge-error-issue-close-reason = issue.close 'reason' must be 'completed' or 'not_planned'.
tool-git-forge-error-pull-merge-method = pull.merge 'method' must be 'merge', 'squash', or 'rebase'.
tool-git-forge-error-review-verdict = review.create 'verdict' must be approve|request_changes|comment, got '{ $verdict }'.
tool-git-forge-error-unknown-cell = unknown or unsupported resource/action '{ $resource }.{ $action }'. Call action 'describe' for the supported grid, or use 'raw' for anything unlisted.
tool-git-forge-error-no-channels = No channels available yet (channels not initialized).
tool-git-forge-error-raw-requires-method = 'raw' requires 'method'.
tool-git-forge-error-raw-requires-path = 'raw' requires 'path'.
tool-git-forge-error-requires-resource = a typed call requires 'resource' (or use action 'raw'/'describe').
tool-git-forge-error-missing-repo = Missing 'repo' (expected 'owner/repo').

tool-glob-search = Search for files matching a glob pattern within the workspace. Returns a sorted list of matching file paths relative to the workspace root. Examples: '**/*.rs' (all Rust files), 'src/**/mod.rs' (all mod.rs in src).

tool-google-workspace = Interact with Google Workspace services (Drive, Gmail, Calendar, Sheets, Docs, etc.) via the gws CLI. Requires gws to be installed and authenticated.

tool-hardware-board-info = Return full board info (chip, architecture, memory map) for connected hardware. Use when: user asks for 'board info', 'what board do I have', 'connected hardware', 'chip info', 'what hardware', or 'memory map'.

tool-hardware-memory-map = Return the memory map (flash and RAM address ranges) for connected hardware. Use when: user asks for 'upper and lower memory addresses', 'memory map', 'address space', or 'readable addresses'. Returns flash/RAM ranges from datasheets.

tool-hardware-memory-read = Read actual memory/register values from Nucleo via USB. Use when: user asks to 'read register values', 'read memory at address', 'dump memory', 'lower memory 0-126', or 'give address and value'. Returns hex dump. Requires Nucleo connected via USB and probe feature. Params: address (hex, e.g. 0x20000000 for RAM start), length (bytes, default 128).

tool-http-request = Make HTTP requests to external APIs. Supports GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS methods. Security constraints: allowlist-only domains, no local/private hosts, configurable timeout and response size limits.

tool-image-info = Read image file metadata (format, dimensions, size) and optionally return base64-encoded data.

tool-jira = Interact with Jira: read tickets, search with JQL, add comments, list projects and per-issue transitions, transition an issue through its workflow, and create new issues.

tool-knowledge = Manage a knowledge graph of architecture decisions, solution patterns, lessons learned, experts, and relationship links.

tool-linkedin = Manage LinkedIn: create posts, list your posts, comment, react, delete posts, view engagement, get profile info, and read the configured content strategy. Requires LINKEDIN_* credentials in .env file.

tool-discord-search = Search Discord message history stored in discord.db. Use to find past messages, summarize channel activity, or look up what users said. Supports keyword search and optional filters: channel_id, since, until.

tool-memory-forget = Remove a memory by key. Use to delete outdated facts or sensitive data. Returns whether the memory was found and removed.

tool-memory-recall = Search long-term memory for relevant facts, preferences, or context. Returns scored results ranked by relevance. Omit the query or pass bare * to return recent memories.

tool-memory-store = Store a fact, preference, or note in long-term memory. Use category 'core' for permanent facts, 'daily' for session notes, 'conversation' for chat context, or a custom category name.

tool-microsoft365 = Microsoft 365 integration: manage Outlook mail, Teams messages, Calendar events, OneDrive files, and SharePoint search via Microsoft Graph API

tool-model-routing-config = Manage default model settings, scenario-based provider/model routes, classification rules, and aliased agent profiles

tool-notion = Interact with Notion: query databases, read/create/update pages, and search the workspace.


tool-project-intel = Project delivery intelligence: generate status reports, detect risks, draft client updates, summarize sprints, and estimate effort. Read-only analysis tool.

tool-proxy-config = Manage ZeroClaw proxy settings (scope: environment | zeroclaw | services), including runtime and process env application

tool-pushover = Send a Pushover notification to your device. Requires PUSHOVER_TOKEN and PUSHOVER_USER_KEY in .env file.

tool-schedule = Manage scheduled shell-only tasks. Actions: create/add/once/list/get/cancel/remove/pause/resume. WARNING: This tool creates shell jobs whose output is only logged, NOT delivered to any channel. To send a scheduled message to Discord/Telegram/Slack/Matrix, use the cron_add tool with job_type='agent' and a delivery config like {"{"}"mode":"announce","channel":"discord","to":"<channel_id>"{"}"}.

tool-screenshot = Capture a screenshot of the current screen. Returns the file path and base64-encoded PNG data.
tool-browser-screenshot-error-path-not-allowed = Screenshot path '{ $path }' is not in the workspace allowlist
tool-browser-screenshot-error-parent-not-exist = Screenshot path '{ $path }' parent directory '{ $parent }' does not exist
tool-browser-screenshot-error-path-outside-workspace = Screenshot path '{ $path }' resolves to '{ $canonical }' which is outside the workspace
tool-browser-screenshot-error-missing-filename = Screenshot path '{ $path }' is missing a filename component
tool-browser-screenshot-error-runtime-config-target = Cannot write screenshot to runtime config path '{ $target }'
tool-browser-screenshot-error-symlink-target = Cannot write screenshot to symlink target '{ $target }'
tool-browser-screenshot-error-path-not-utf8 = Screenshot path '{ $path }' resolves to a non-UTF-8 pathname; refusing to write through a lossy conversion
tool-browser-screenshot-error-computeruse-non-string-path = Screenshot 'path' parameter must be a string, got { $path }
tool-browser-screenshot-error-non-string-path = Screenshot 'path' must be a string or absent
tool-browser-screenshot-error-args-not-object = Screenshot arguments must be a JSON object
tool-browser-screenshot-error-sidecar-no-png-data = computer-use sidecar did not return PNG data
tool-browser-screenshot-error-sidecar-empty-png = computer-use sidecar returned an empty screenshot payload
tool-browser-screenshot-error-sidecar-not-png = computer-use sidecar returned a non-PNG screenshot payload
tool-browser-screenshot-error-sidecar-non-json-success = computer-use sidecar returned a non-JSON success response for a path-bearing screenshot; the requested file was not written

tool-security-ops = Security operations tool for managed cybersecurity services. Actions: triage_alert (classify/prioritize alerts), run_playbook (execute incident response steps), parse_vulnerability (parse scan results), generate_report (create security posture reports), list_playbooks (list available playbooks), alert_stats (summarize alert metrics).

tool-shell = Execute a shell command in the workspace directory

tool-sop-advance = Report the result of the current SOP step and advance to the next step. Provide the run_id, whether the step succeeded or failed, and a brief output summary.

tool-sop-approve = Approve a pending SOP step that is waiting for operator approval. Returns the step instruction to execute. Use sop_status to see which runs are waiting.

tool-sop-execute = Manually trigger a Standard Operating Procedure (SOP) by name. Returns the run ID and first step instruction. Use sop_list to see available SOPs.

tool-sop-list = List all loaded Standard Operating Procedures (SOPs) with their triggers, priority, step count, and active run count. Optionally filter by name or priority.

tool-sop-status = Query SOP execution status. Provide run_id for a specific run, or sop_name to list runs for that SOP. With no arguments, shows all active runs.

tool-tool-search = Fetch full schema definitions for deferred MCP tools so they can be called. Use "select:name1,name2" for exact match or keywords to search.

tool-web-fetch = Fetch a web page and return its content as clean plain text. HTML pages are automatically converted to readable text. JSON and plain text responses are returned as-is. Only GET requests; follows redirects. Security: allowlist-only domains, no local/private hosts.

tool-web-search-tool = Search the web for information. Returns relevant search results with titles, URLs, and descriptions. Use this to find current information, news, or research topics.
tool-web-search-tool-error-duckduckgo-blocked = DuckDuckGo is rate-limiting this machine. Do not retry or rephrase the search; wait a few minutes, fetch known URLs directly with web_fetch, or configure SearXNG, Brave, or Tavily as the web_search provider.
tool-web-search-tool-error-searxng-not-configured = SearXNG instance URL not configured. Set [web_search] searxng_instance_url in config.toml, or override it with the ZEROCLAW_web_search__searxng_instance_url environment variable.
tool-web-search-tool-note-truncated-results = (further results omitted)

tool-workspace = Manage multi-client workspaces. Subcommands: list, switch, create, info, export. Each workspace provides isolated memory, audit, secrets, and tool restrictions.

tool-weather = Get current weather conditions and forecast for any location worldwide. Supports city names (in any language or script), IATA airport codes (e.g. 'LAX'), GPS coordinates (e.g. '51.5,-0.1'), postal/zip codes, and domain-based geolocation. Returns temperature, feels-like, humidity, wind speed/direction, precipitation, visibility, pressure, UV index, and cloud cover. Optional 0-3 day forecast with hourly breakdown. Units default to metric (°C, km/h, mm) but can be set to imperial (°F, mph, inches) per request. No API key required.

# --- coverage completion: descriptions for tools that previously fell back to English ---

# delegation / CLI coding agents / subagents
tool-claude-code = Delegate a coding task to Claude Code (claude -p). Supports file editing, bash execution, structured output, and multi-turn sessions. Use for complex coding work that benefits from Claude Code's full agent loop.
tool-claude-code-runner = Spawn a Claude Code task in a tmux session with live Slack progress updates and SSH handoff. Returns immediately with session ID and attach command.
tool-codex-cli = Delegate a coding task to Codex CLI (codex exec). Supports file editing and bash execution. Use for complex coding work that benefits from Codex's full agent loop.
tool-gemini-cli = Delegate a coding task to Gemini CLI (gemini -p). Supports file editing and shell execution. Use for complex coding work that benefits from Gemini CLI's full agent loop.
tool-opencode-cli = Delegate a coding task to OpenCode CLI (opencode run). Supports file editing and bash execution. Use for complex coding work that benefits from OpenCode's full agent loop.
tool-spawn-subagent = Spawn an ephemeral SubAgent that inherits this agent's identity, security policy, and memory allowlist. The SubAgent runs the supplied prompt to completion under the parent's permissions envelope and returns its response. Use for focused subtasks (research lookup, multi-step reasoning, etc.) that should not pollute this agent's main conversation history. Cost-aware: each SubAgent run is a full agent loop and consumes provider tokens.
tool-llm-task = Run a prompt through an LLM with no tool access and return the response. Optionally validates the output against a JSON Schema. Ideal for structured data extraction, classification, summarization, and transformation tasks.

# pipeline / runtime model switch
tool-execute-pipeline = Execute a multi-step tool pipeline in a single call. Steps run sequentially by default with result interpolation (use {"{{step[N].result}}"} to reference prior outputs), or in parallel when 'parallel: true' is set. Set 'result: "last"' to return only the final step's output (recommended when an earlier step yields a large blob, e.g. base64, that should not flow back into the context); the default 'all' returns every step's result.
tool-model-switch = Request a runtime model switch using a configured provider profile plus provider-local model. Use 'get' to see the pending switch, 'list_model_providers' to see provider families, 'list_models' to see common models for a provider profile, or 'set' with a dotted provider profile ref such as 'openai.default'. The switch is runtime/session state and does not write config.

# interaction / compute / canvas / todo
tool-ask-user = Ask the user a question and wait for their response. Sends the question to a messaging channel and blocks until the user replies or the timeout expires. Optionally provide choices for structured responses.
tool-escalate-to-human = Escalate a situation to a human operator with urgency routing. Sends a structured message to the active channel. High/critical urgency also notifies any channels listed in `[escalation] alert_channels`, which additionally serve as a fallback when the active channel cannot deliver. Optionally blocks to wait for a human response.
tool-calculator = Perform arithmetic and statistical calculations. Supports 25 functions: add, subtract, divide, multiply, pow, sqrt, abs, modulo, round, log, ln, exp, factorial, sum, average, median, mode, min, max, range, variance, stdev, percentile, count, percentage_change, clamp. Use this tool whenever you need to compute a numeric result instead of guessing.
tool-canvas = Push rendered content (HTML, SVG, Markdown) to a live web canvas that users can see in real-time. Actions: render (push content), snapshot (get current content), clear (reset canvas), eval (evaluate JS expression in canvas context). Each canvas is identified by a canvas_id string.
tool-TodoWrite = Render a live task tracker for the current work. Call this with the COMPLETE current todo list every time — the new list wholly replaces the previous one. Each todo has `content` (imperative description), `status` (pending, in_progress, or completed), and optionally `priority` (high, medium, low) and `activeForm` (present-continuous label shown while in_progress). Keep exactly one item in_progress at a time. Pass an empty list to clear the tracker.

# messaging / delivery
tool-send-via = Control where and how this turn's reply is delivered, or send an extra message to another channel. WHEN TO USE: call this tool at the start of your response whenever the user requests a specific reply format or destination — e.g. "reply by text", "send as voice", "text only", "send to my email", "redirect to Discord". Do not wait for the user to name the tool; infer intent from natural language just as you would use a weather tool when asked for the weather. Without `body` (routing instruction — affects this turn's main reply): - `send_via(modality: "text")` — reply by text even on a voice-only peer - `send_via(modality: "voice")` — reply by voice even on a text-only peer - `send_via(target: "discord.main")` — redirect reply to another channel - `send_via(target: "discord.main", modality: "voice")` — redirect + force modality At least one of `target` or `modality` is required when `body` is absent. With `body` (immediate fanout — main reply still goes to originating channel): - `send_via(target: "email.default", body: "...")` — send separate content elsewhere `target` is required when `body` is present. `target` must be a channel alias (e.g. `telegram.default`) or a peer group name the active agent belongs to. `modality` defaults to the peer group's output_modality.
tool-send-message-to-peer = Send a message to a peer agent or external peer (human, external bot) on a shared channel. The target must be a member of a peer group both this agent and the target agree on (or an external peer listed on the shared group's `external_peers`). Cross-agent sends to non-peers are rejected at the tool boundary; the channel send only happens after the peer-set check passes. Use the current channel ref unless the user explicitly names another allowed channel. Do not pass peer group names as the `channel` parameter.
tool-deliver-file = Deliver a file from the workspace to the ACP client as an embedded binary resource (PDF, DOCX, images, etc.). Use when the user should download or preview the file. Path must stay inside the workspace. On success the result includes `uri` (`attachment://deliver/<content-hash>`) — cite that exact uri in widgets/`[N]`; do not invent prefixes. Pass an optional `title` (any prose) as the client's chat label for the file; it defaults to the filename. Do not invent ACP filename fields.

# email
tool-email-read = Fetch the full content of an email by its UID (from email_search results). Returns sender, subject, date, body text, and attachment names. Never marks the email as read.
tool-email-search = Search emails in a configured IMAP mailbox. Never modifies any email (read-state is preserved). Use to check if someone sent a message, find emails by subject, or look up threads. Returns sender, subject, date, and UID for each match.

# files / upload / images
tool-file-upload = Upload a local file to the configured remote endpoint via multipart/form-data. The file path stays on the host; bytes are not loaded into model context. Returns the HTTP status and a truncated response body so the caller can extract any URL or identifier the receiver echoes back.
tool-file-upload-bundle = Upload N local files as a single multipart/form-data request. All files are sent in one HTTP round-trip; however, transactional (all-or-nothing) semantics depend on the receiving endpoint. Use for multi-file deliverables (HTML + CSS + JS, report + figures). File paths stay on the host; bytes are not loaded into model context. Returns the HTTP status and a truncated response body.
tool-image-gen = Generate an image from a text prompt using fal.ai (Flux models). Saves the result to the workspace images directory and returns the file path.

# git forge
tool-git-forge = Operate on a git forge (GitHub/Gitea) through the git channel. Actions: 'describe' returns the resource/action grid and endpoint shapes; a typed call takes {"{resource, action, repo, ...}"} for milestone/label/issue/pull/ review/reviewer/comment (validated beyond a bare 2xx); 'raw' takes {"{method, path, body}"} for any endpoint not yet typed. Call 'describe' first when unsure. Names the git channel by its channel key (default 'git').
tool-git = Operate on a git forge (GitHub/Gitea) through the git channel. Actions: 'describe' returns the resource/action grid and endpoint shapes; a typed call takes {"{resource, action, repo, ...}"} for milestone/label/issue/pull/ review/reviewer/comment (validated beyond a bare 2xx); 'raw' takes {"{method, path, body}"} for any endpoint not yet typed. Call 'describe' first when unsure. Names the git channel by its channel key (default 'git').

# mcp prompts / resources
tool-mcp-prompts = List or get prompts exposed by connected MCP servers. action=list [server,cursor] returns available prompts (names are prefixed `<server>__<name>`); action=get name=<prefixed-name> arguments={"{...}"} returns the resolved prompt messages.
tool-mcp-resources = List or read resources exposed by connected MCP servers. action=list [server,cursor] returns available resources (uris are prefixed `<server>__<uri>`); action=read uri=<prefixed-uri> returns the resource contents.

# memory
tool-memory-export = Export visible memories as a JSON array for GDPR Art. 20 data portability. Supports filtering by namespace, session, category, and time range. Returns a structured, machine-readable JSON array of entries that pass the active memory read policy.
tool-memory-purge = Remove all memories in a namespace or session. Use to bulk-delete per-tenant or per-conversation data. Returns the number of deleted entries. WARNING: This operation cannot be undone.

# sessions
tool-sessions-list = List all active conversation sessions with their channel, last activity time, and message count.
tool-sessions-history = Read the message history of a specific session by its session ID. Returns the last N messages.
tool-sessions-send = Send a message to a specific session by its session ID. The message is appended to the session's conversation history as a 'user' message, enabling inter-agent communication.
tool-sessions-current = Return the session key and metadata for the session this agent is currently running in.
tool-sessions-reset = Reset a session by clearing all its messages. The session can still receive new messages after reset.
tool-sessions-delete = Permanently delete a session and all its messages. This cannot be undone.

# skills / SOP
tool-read-skill = Read the full source file for an available skill by name. Use this in compact skills mode when you need the complete skill instructions without remembering file paths.
tool-skills-list = List installed skills with their name, version, and one-line description. Read-only. Use before `skill_view` or `skill_manage` to find candidate slugs.
tool-skill-view = Read a single skill's SKILL.md content (YAML front-matter + body preview) plus the names of its support files under references/, templates/, scripts/. Use this before deciding whether to patch the skill or add a support file.
tool-skill-manage = Mutating operations on installed skills. Actions: `patch` (atomically rewrite SKILL.md — supply the full new file content; the YAML front-matter must have a `name` field), `write_file` (add a file under references/, templates/, or scripts/), `archive` (move to .archive/). All writes go through atomic temp-rename and validation where applicable.
tool-sop-workshop = Manage SOP procedural-memory proposals: propose, capture_run, list, inspect, apply, reject, or quarantine. Apply writes SOP.toml/SOP.md only after an explicit action.

# browsing
tool-text-browser = Render a web page as plain text using a text-based browser (lynx, links, or w3m). Ideal for headless/SSH environments without a graphical browser. Auto-detects available browser or uses a configured preference. For untrusted URLs, prefer web_fetch because external browsers can re-resolve DNS and follow redirects to unvalidated hosts.
