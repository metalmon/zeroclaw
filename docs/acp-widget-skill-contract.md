# ACP Widget-Skill Contract (Thunderbolt ⇆ ZeroClaw)

**Status:** agreed design, 2026-07-30
**Scope:** how a Thunderbolt-style ACP client delivers "skills" and client-executed
capabilities (e.g. `ask`, `map`, `say`) to a ZeroClaw agent over ACP, and what each
side is responsible for.

This document is the shared source of truth. Hand it to whoever implements the
Thunderbolt side; the ZeroClaw side is tracked in this repo.

---

## 1. The model

Thunderbolt does **not** send executable tool specs to the agent and the agent does
**not** run client tools. Instead:

1. **Skills are instruction-only.** The client advertises skills as free-text
   instructions. The model loads a skill's instruction (progressive disclosure) and
   follows it.
2. **Client capabilities are widgets emitted as inline markup.** A skill's
   instruction tells the model to emit a self-closing tag like
   `<widget:NAME attr="..." />` **in its normal assistant text**. The **client**
   parses that markup out of the streamed text and renders / executes it
   (renders a UI widget, shows a map, speaks text). There is no tool call, no
   tool registry entry, and no agent→client execution RPC for these.
3. **The agent is a passthrough** for widget markup. It streams the model's text —
   including any `<widget:…>` tags — to the client unchanged.

Consequences:
- No client-provided tools, no MCP-over-ACP, no bespoke `tool/execute` callback are
  required for `ask` / `map` / `say`.
- `ask` (interactive) needs no result RPC: the user's answer arrives as the next
  normal chat message. `say` is fire-and-forget.
- `session/request_permission` remains **only** for approving the agent's *own real*
  tools, and is unrelated to widgets.

---

## 2. ACP wire contract

### 2.1 Namespace
`_meta` extension key: **`thunderbird.net/thunderbolt`**.

### 2.2 Capability (agent → client, `initialize` response)
Agent advertises support for wire-delivered skills:
```json
{ "_meta": { "thunderbird.net/thunderbolt": { "skills": true } } }
```

### 2.3 Skills payload (client → agent, `session/new` | `session/resume` | `session/load`)
```json
{ "_meta": { "thunderbird.net/thunderbolt": { "skills": [ SkillDefinition, ... ] } } }
```
`SkillDefinition` — **exactly** three fields, all strings:
```ts
type SkillDefinition = { name: string; description: string; instruction: string }
```
No `tools`, no `prompts`, no `kind`/`command`/`args`/`target`. (Thunderbolt source of
truth: `shared/agent-core/skills.ts`, merged in Thunderbolt PR #1137 "convert chat
widgets into skills with progressive disclosure over ACP".)

The agent lists `name`/`description` in its system prompt and returns `instruction`
on demand via its `skill`/`read_skill` tool.

---

## 3. Widget markup convention

The model emits a self-closing tag inside its assistant text:
```
<widget:NAME attr1="VALUE" attr2='JSON_ARRAY' />
```
- Attribute values are double-quoted; JSON-array values use single quotes.
- One widget tag per prompt/action. The client extracts widgets from the text
  (`extractWidgets(text)` on the Thunderbolt side) and renders/acts on them.
- Each widget is defined on the **client**: `src/widgets/<name>/{schema,display,instructions}`.

### 3.1 Existing widgets (reference)
`ask` (interactive prompt):
```
<widget:ask mode="single" prompt="Which protocol sends outgoing mail?"
  options='[{"id":"a","text":"SMTP","isCorrect":true},{"id":"b","text":"IMAP"}]'
  explanation="SMTP sends; IMAP/POP3 retrieve." />
```
The user's response returns as the next normal chat message. `map`, `link-preview`,
`weather-forecast`, etc. follow the same shape.

### 3.2 `say` widget (to be defined on the Thunderbolt side)
Recommended shape (Thunderbolt owns the final schema):
```
<widget:say text="The text to speak aloud." />
```
- Fire-and-forget: the client's voice engine speaks `text`; no result is returned to
  the agent.
- Ship a default `say` skill whose `instruction` teaches the model when and how to
  emit `<widget:say>` (model this on `defaultSkillAsk` + `src/widgets/ask/instructions.ts`).
- Client work: `src/widgets/say/{schema,instructions,parser/executor}` + register it in
  the widget extractor and skill defaults.

---

## 4. Responsibilities

**Thunderbolt (client) owns:**
- Defining widgets (`say`, etc.): schema, instruction, and the client-side
  parse+execute (speak) path.
- Shipping the corresponding skill instruction via the `_meta` skills payload.
- Advertising `mcpServers` only for *actual external* MCP servers (unrelated to widgets).

**ZeroClaw (agent) owns:**
- Advertising `{ skills: true }` and reading `SkillDefinition[]` from session `_meta`
  (already implemented).
- Progressive disclosure of `instruction` via the `skill`/`read_skill` tool (already
  implemented).
- **Guaranteeing widget-markup passthrough:** `<widget:…>` tags in assistant text must
  survive the *entire* outbound pipeline (streaming deltas, `on_message_sending`
  hook, credential leak-detection, format sanitization, tool-call/think stripping)
  and reach the ACP client byte-for-byte. No stripping, no HTML-escaping of `<`/`>`,
  no reflow that splits a tag.

**Neither side needs:** client-advertised executable tools, MCP-over-ACP, or an
agent→client tool-execution RPC for widgets.

---

## 5. ZeroClaw cleanup

The current `WireSkill` on the agent carries speculative `tools: Vec<WireSkillTool>`
and `prompts` fields (with `kind`/`command`/`args`/`target`) that model
**agent-side** tool execution. Thunderbolt never sends them, and they do not fit the
widget model (which is client-side, markup-driven). They are removed so the wire type
matches `SkillDefinition` and no ACP client can inject an agent-executed
`kind:"shell"` command.
