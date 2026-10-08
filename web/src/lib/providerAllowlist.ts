// Client-side MODEL-PROVIDER allowlist for the pilot / RU (Volt) build, used to
// trim the /integrations catalog's AiModel category (77 providers upstream).
// Cosmetic UI filter: the daemon still supports every provider — this only
// trims what the catalog SHOWS. Matched by provider DISPLAY NAME (the
// /api/integrations entries carry no canonical key), case-insensitive.
//
// `VITE_VOLT_PROVIDERS` (build-time env) = comma-separated provider display
// names to KEEP. Empty / unset = passthrough (show every provider).
const raw = String(import.meta.env.VITE_VOLT_PROVIDERS ?? "").trim();
const allow: Set<string> | null = raw
  ? new Set(
      raw
        .split(",")
        .map((s: string) => s.trim().toLowerCase())
        .filter(Boolean),
    )
  : null;

/** True when a model provider (by display name) should be shown. */
export function isProviderAllowed(displayName: string): boolean {
  return allow === null || allow.has(displayName.trim().toLowerCase());
}
