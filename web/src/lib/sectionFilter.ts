// Client-side SECTION hide-list for the pilot / RU (Volt) build. Cosmetic UI
// filter: the daemon's config schema still defines every section — this only
// trims which ones the panel's settings navigator SHOWS. Unlike the
// channel/tool allowlists (keep-lists), this is a HIDE-list: there are dozens
// of sections and we only want to remove a few integrations (Jira, Notion,
// LinkedIn, …) that aren't part of the RU offering.
//
// `VITE_VOLT_HIDE_SECTIONS` (build-time env) = comma-separated section keys to
// HIDE. Empty / unset = show every section.
const raw = String(import.meta.env.VITE_VOLT_HIDE_SECTIONS ?? "").trim();
const hidden: Set<string> = new Set(
  raw
    ? raw
        .split(",")
        .map((s: string) => s.trim())
        .filter(Boolean)
    : [],
);

/** True when config section `key` should be hidden from the panel. */
export function isSectionHidden(key: string): boolean {
  return hidden.has(key);
}

/** Drop hidden sections from a list of `{ key }`. Unchanged when nothing is hidden. */
export function filterSections<T extends { key: string }>(items: T[]): T[] {
  return hidden.size === 0 ? items : items.filter((i) => !hidden.has(i.key));
}
