// Client-side TOOL allowlist for the pilot / RU build. Cosmetic UI filter:
// the daemon still exposes every tool — this only trims what the panel's
// tool pickers and catalog SHOW (the real capability gate is Stage-2). Mirrors
// the channel/provider allowlist idea.
//
// `VITE_VOLT_TOOLS` (build-time env) = comma-separated tool names to KEEP.
// Empty / unset = passthrough (show every tool, no filtering).
const raw = String(import.meta.env.VITE_VOLT_TOOLS ?? '').trim();
const allow: Set<string> | null = raw
  ? new Set(raw.split(',').map((s: string) => s.trim()).filter(Boolean))
  : null;

/** True when `name` should be shown (always true in passthrough mode). */
export function isToolAllowed(name: string): boolean {
  return allow === null || allow.has(name);
}

/** Filter a catalog (any list of `{ name }`) down to the allowlisted tools.
 *  Returns the input unchanged in passthrough mode. */
export function filterToolCatalog<T extends { name: string }>(entries: T[]): T[] {
  return allow === null ? entries : entries.filter((e) => isToolAllowed(e.name));
}
