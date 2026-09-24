// Build-time allowlist for the RU pilot: hides non-RU channels/providers from
// the panel's dynamic pickers. Sourced from Vite env, comma-separated. Empty or
// unset => no filtering (upstream behavior). This is a UI-layer cosmetic filter;
// the daemon API still serves the hidden paths (a real gate is a later stage).
function parseEnv(v: string | undefined): string[] | null {
  if (!v) return null;
  const list = v.split(",").map((s) => s.trim()).filter(Boolean);
  return list.length ? list : null;
}
export function channelAllowlist(): string[] | null {
  return parseEnv(import.meta.env.VITE_VOLT_CHANNELS as string | undefined);
}
export function providerAllowlist(): string[] | null {
  return parseEnv(import.meta.env.VITE_VOLT_PROVIDERS as string | undefined);
}
/** Keep only items whose key is in `allow`; null/empty allow = passthrough. */
export function filterByAllow<T>(items: T[], key: (t: T) => string, allow: string[] | null): T[] {
  if (!allow || allow.length === 0) return items;
  const set = new Set(allow.map((s) => s.toLowerCase()));
  return items.filter((it) => set.has(key(it).toLowerCase()));
}
