// Client-side CHANNEL allowlist for the pilot / RU build. Cosmetic UI filter:
// the daemon's config schema still defines every channel type — this only
// trims which ones the panel's channel picker OFFERS to add (the real gate is
// the daemon's compiled channel features + Stage-2). Mirrors the tool allowlist.
//
// `VITE_VOLT_CHANNELS` (build-time env) = comma-separated channel keys to KEEP.
// Empty / unset = passthrough (offer every channel type).
const raw = String(import.meta.env.VITE_VOLT_CHANNELS ?? '').trim();
const allow: Set<string> | null = raw
  ? new Set(raw.split(',').map((s: string) => s.trim()).filter(Boolean))
  : null;

/** True when channel `key` should be offered (always true in passthrough mode). */
export function isChannelAllowed(key: string): boolean {
  return allow === null || allow.has(key);
}

/** Filter channel picker items (any list of `{ key }`) to the allowlist.
 *  Returns the input unchanged in passthrough mode. */
export function filterChannelItems<T extends { key: string }>(items: T[]): T[] {
  return allow === null ? items : items.filter((i) => isChannelAllowed(i.key));
}
