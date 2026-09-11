// Guards the hand-maintained i18n catalog (src/lib/i18n.ts + src/locales/*.ts)
// against drift while translation of the RU catalog is in progress:
//
//   a) key coverage   — every EN key under an allowlisted prefix must exist in RU
//   b) placeholder    — `{token}` sets must match between EN and RU for shared keys
//   c) split-key      — paired `_prefix`/`_suffix` siblings must be translated together
//   d) english-leak   — allowlisted RU values that are still ~all-ASCII (untranslated)
//
// Keys outside the allowlist are reported as INFO only, so partial translation
// stays green. Run standalone: `node scripts/check-i18n.mjs`.

import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createJiti } from "jiti";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");
const I18N_PATH = resolve(repoRoot, "src/lib/i18n.ts");
const RU_PATH = resolve(repoRoot, "src/locales/ru.ts");
const NAMESPACES_PATH = resolve(repoRoot, "scripts/translated-namespaces.txt");
const SCHEMA_PATHS_PATH = resolve(repoRoot, "scripts/schema-paths.json");

const SPLIT_SUFFIXES = ["_prefix", "_suffix"];
const PLACEHOLDER_RE = /\{[^{}]*\}/g;
const MIN_LEAK_LETTERS = 4;
const LEAK_ASCII_RATIO = 0.95;

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/**
 * Extract a top-level `<key>: { ... }` object-literal value out of a TS/JS
 * source string without executing (or even parsing as a module) the rest of
 * the file. i18n.ts transitively imports browser-only modules (`window`),
 * so a real module import isn't viable in a plain Node script; this walks
 * the source text tracking string/brace state so `{placeholder}` tokens
 * inside translation values don't confuse the brace matcher.
 */
export function extractObjectLiteral(source, key) {
  const marker = new RegExp(`(^|\\n)\\s*${key}\\s*:\\s*\\{`);
  const match = marker.exec(source);
  if (!match) {
    throw new Error(`extractObjectLiteral: could not find "${key}: {" block`);
  }
  const openBrace = match.index + match[0].length - 1;
  let depth = 0;
  let quote = null; // "'" | '"' | "`" | null
  let i = openBrace;
  for (; i < source.length; i++) {
    const ch = source[i];
    if (quote) {
      if (ch === "\\") {
        i++;
        continue;
      }
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === "`") {
      quote = ch;
      continue;
    }
    if (ch === "{") {
      depth++;
    } else if (ch === "}") {
      depth--;
      if (depth === 0) {
        i++;
        break;
      }
    }
  }
  if (depth !== 0) {
    throw new Error(`extractObjectLiteral: unbalanced braces in "${key}" block`);
  }
  const literalText = source.slice(openBrace, i);
  return new Function(`"use strict"; return (${literalText});`)();
}

export function loadEnCatalog(i18nPath = I18N_PATH) {
  const source = readFileSync(i18nPath, "utf8");
  return extractObjectLiteral(source, "en");
}

export async function loadRuCatalog(ruPath = RU_PATH) {
  const jiti = createJiti(import.meta.url);
  const mod = await jiti.import(pathToFileURL(ruPath).href, { default: false });
  return mod.ru;
}

export function loadAllowlist(path = NAMESPACES_PATH) {
  if (!existsSync(path)) return [];
  return readFileSync(path, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
}

export function loadSchemaPaths(path = SCHEMA_PATHS_PATH) {
  if (!existsSync(path)) return [];
  return JSON.parse(readFileSync(path, "utf8"));
}

// ---------------------------------------------------------------------------
// Config field catalog vs. schema fixture ("schema-audit")
// ---------------------------------------------------------------------------
//
// `config.field.<normpath>.label` / `.desc` catalog entries (src/locales/*.ts)
// are keyed by the NORMALIZED schema path: a map/list instance key (agent
// alias, principal id, role id, ...) collapses to `*` so one entry covers
// every runtime instance of that map — see `fieldLabel`/`fieldDesc` in
// src/lib/i18n.ts. This mirrors `normalizeConfigFieldPath` from that file;
// it is re-implemented here rather than imported because i18n.ts transitively
// touches `window` at module scope (same reason check-i18n.test.mjs
// re-implements plural() instead of importing it — see that file's comment).
// Keep DYNAMIC_KEY_SECTIONS in sync with lib/i18n.ts by hand when either
// changes.
const DYNAMIC_KEY_SECTIONS = [
  { prefix: "agents.", keySegmentIndex: 1 },
  { prefix: "authz.principals.", keySegmentIndex: 2 },
  { prefix: "authz.profiles.", keySegmentIndex: 2 },
];

export function normalizeConfigFieldPath(path) {
  const segments = path.split(".");
  for (const { prefix, keySegmentIndex } of DYNAMIC_KEY_SECTIONS) {
    if (path.startsWith(prefix) && segments.length > keySegmentIndex) {
      segments[keySegmentIndex] = "*";
    }
  }
  return segments.join(".");
}

const FIELD_KEY_RE = /^config\.field\.(.+)\.(label|desc)$/;

/**
 * Extract the set of normalized schema paths a `config.field.*` catalog
 * (en ∪ ru, or any locale) declares entries for.
 */
export function fieldCatalogPaths(...catalogs) {
  const paths = new Set();
  for (const catalog of catalogs) {
    for (const key of Object.keys(catalog)) {
      const m = FIELD_KEY_RE.exec(key);
      if (m) paths.add(m[1]);
    }
  }
  return paths;
}

/**
 * WARN (never fails the build) on `config.field.*` catalog entries whose
 * normalized path does not correspond to any path in the live schema
 * fixture (schema-paths.json) — i.e. a typo, or a field the upstream Rust
 * schema has since renamed/removed (schema drift). `schemaPaths` are the
 * RAW (runtime-instance) paths from the fixture; they're normalized here
 * with the same function the catalog keys are supposed to follow.
 */
export function checkFieldCatalogOrphans(catalogPaths, schemaPaths) {
  const known = new Set(schemaPaths.map(normalizeConfigFieldPath));
  const warnings = [];
  for (const path of catalogPaths) {
    if (!known.has(path)) {
      warnings.push(`orphan: "config.field.${path}" has no matching schema path (typo or schema drift?)`);
    }
  }
  return warnings;
}

/**
 * WARN (never fails the build) with a per-prefix count of normalized schema
 * paths under a "high-traffic" prefix that have no `config.field.*.label`
 * entry yet. Purely a backlog-visibility signal — translation of the config
 * schema is deliberately partial (see the catalog's own header comment), so
 * this never gates the build; it just makes the remaining gap under the
 * prefixes we've decided matter most visible instead of silent.
 */
export function checkHighTrafficFieldCoverage(catalogPaths, schemaPaths, prefixes) {
  const normalizedSchema = [...new Set(schemaPaths.map(normalizeConfigFieldPath))];
  const warnings = [];
  for (const prefix of prefixes) {
    const underPrefix = normalizedSchema.filter((p) => p.startsWith(prefix));
    const missing = underPrefix.filter((p) => !catalogPaths.has(p));
    if (missing.length > 0) {
      warnings.push(
        `high-traffic gap: ${missing.length}/${underPrefix.length} field(s) under "${prefix}" have no config.field.*.label (e.g. "${missing[0]}")`,
      );
    }
  }
  return warnings;
}

// High-traffic prefixes per the RU panel plan's field-catalog fill scope —
// kept here (not derived) so this stays a deliberate, reviewable list, same
// spirit as translated-namespaces.txt.
export const HIGH_TRAFFIC_FIELD_PREFIXES = [
  "gateway.",
  "authz.principals.*.",
  "authz.profiles.*.",
  "agents.*.",
];

// ---------------------------------------------------------------------------
// Checks — each returns { failures: string[], infos: string[] }
// ---------------------------------------------------------------------------

function isAllowlisted(key, prefixes) {
  return prefixes.some((prefix) => key.startsWith(prefix));
}

const PLURAL_REQUIRED_RU_SUFFIXES = ["_one", "_few", "_many"];

/**
 * Find the set of "plural bases" in a catalog — key prefixes that have at
 * least one CLDR plural-category child (`<base>_one`, `<base>_few`,
 * `<base>_many`, `<base>_other`). Detection is anchored on `_other` because
 * every plural family in this catalog carries it (English's PluralRules
 * only ever selects "one" or "other", so a real family always defines
 * `_other` as its fallback); this keeps a key that merely happens to end in
 * `_one` for unrelated reasons (e.g. "click to add one") from being
 * misdetected as a plural base with no sibling `_other`.
 */
export function findPluralBases(catalog) {
  const bases = new Set();
  for (const key of Object.keys(catalog)) {
    if (key.endsWith("_other")) {
      bases.add(key.slice(0, -"_other".length));
    }
  }
  return bases;
}

/**
 * Russian needs three plural categories (one/few/many) where English only
 * has two (one/other) — see Intl.PluralRules('ru'). Any EN key that is part
 * of a plural family (has a `_other` sibling) must ship ru forms for all
 * three ru-specific categories, regardless of the coverage allowlist: a
 * missing category isn't merely untranslated, it's a grammar bug (wrong
 * number agreement) for every count that selects it.
 */
export function checkPluralCoverage(en, ru) {
  const failures = [];
  for (const base of findPluralBases(en)) {
    for (const suffix of PLURAL_REQUIRED_RU_SUFFIXES) {
      const key = `${base}${suffix}`;
      if (!Object.prototype.hasOwnProperty.call(ru, key)) {
        failures.push(`plural: "${base}" is missing required ru form "${key}"`);
      }
    }
  }
  return { failures };
}

export function checkCoverage(en, ru, prefixes) {
  const failures = [];
  const infos = [];
  for (const key of Object.keys(en)) {
    if (Object.prototype.hasOwnProperty.call(ru, key)) continue;
    if (isAllowlisted(key, prefixes)) {
      failures.push(`coverage: "${key}" is allowlisted but missing from ru`);
    } else {
      infos.push(`untranslated: "${key}" not yet in ru`);
    }
  }
  return { failures, infos };
}

export function extractTokens(value) {
  return new Set(value.match(PLACEHOLDER_RE) ?? []);
}

function sameTokenSet(a, b) {
  if (a.size !== b.size) return false;
  for (const token of a) if (!b.has(token)) return false;
  return true;
}

export function checkPlaceholderParity(en, ru) {
  const failures = [];
  for (const key of Object.keys(en)) {
    if (!Object.prototype.hasOwnProperty.call(ru, key)) continue;
    const enTokens = extractTokens(en[key]);
    const ruTokens = extractTokens(ru[key]);
    if (!sameTokenSet(enTokens, ruTokens)) {
      failures.push(
        `placeholder: "${key}" token mismatch — en=[${[...enTokens].join(", ")}] ru=[${[...ruTokens].join(", ")}]`,
      );
    }
  }
  return { failures };
}

export function checkSplitKeyOrphans(en, ru) {
  const failures = [];
  const [prefixSuffix, suffixSuffix] = SPLIT_SUFFIXES;
  for (const key of Object.keys(en)) {
    if (!key.endsWith(prefixSuffix)) continue;
    const base = key.slice(0, -prefixSuffix.length);
    const sibling = base + suffixSuffix;
    if (!Object.prototype.hasOwnProperty.call(en, sibling)) continue; // not a real pair
    const hasKey = Object.prototype.hasOwnProperty.call(ru, key);
    const hasSibling = Object.prototype.hasOwnProperty.call(ru, sibling);
    if (hasKey !== hasSibling) {
      const translated = hasKey ? key : sibling;
      const orphan = hasKey ? sibling : key;
      failures.push(
        `split-key: "${translated}" is translated in ru but its paired key "${orphan}" is not`,
      );
    }
  }
  return { failures };
}

const LETTER_RE = /\p{L}/gu;
const ASCII_LETTER_RE = /[A-Za-z]/;
const URL_RE = /https?:\/\/\S+/gi;

/** True when `value` looks like it was left in English (or otherwise untranslated). */
export function looksLikeEnglishLeak(value) {
  if (typeof value !== "string") return false;
  const trimmed = value.trim();
  if (trimmed.length === 0) return false;
  if (!/\s/.test(trimmed)) return false; // single token (URL, %s, {token}, abbreviation) — allow

  // Placeholders and bare URLs aren't translatable content — strip them before
  // judging whether what's left still reads as English (otherwise a
  // format-only value like "{count} / {total}" is flagged purely on the
  // ASCII letters inside its own placeholder names).
  const stripped = trimmed.replace(PLACEHOLDER_RE, " ").replace(URL_RE, " ");

  const letters = stripped.match(LETTER_RE) ?? [];
  if (letters.length < MIN_LEAK_LETTERS) return false; // too short/symbolic to judge, or nothing left
  const asciiLetters = letters.filter((c) => ASCII_LETTER_RE.test(c));
  return asciiLetters.length / letters.length >= LEAK_ASCII_RATIO;
}

export function checkEnglishLeak(ru, prefixes) {
  const failures = [];
  for (const key of Object.keys(ru)) {
    if (!isAllowlisted(key, prefixes)) continue;
    if (looksLikeEnglishLeak(ru[key])) {
      failures.push(`english-leak: "${key}" = ${JSON.stringify(ru[key])} looks untranslated`);
    }
  }
  return { failures };
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

export function runChecks(en, ru, prefixes) {
  const coverage = checkCoverage(en, ru, prefixes);
  const placeholder = checkPlaceholderParity(en, ru);
  const splitKey = checkSplitKeyOrphans(en, ru);
  const englishLeak = checkEnglishLeak(ru, prefixes);
  const pluralCoverage = checkPluralCoverage(en, ru);
  return {
    failures: [
      ...coverage.failures,
      ...placeholder.failures,
      ...splitKey.failures,
      ...englishLeak.failures,
      ...pluralCoverage.failures,
    ],
    infos: [...coverage.infos],
  };
}

async function main() {
  const en = loadEnCatalog();
  const ru = await loadRuCatalog();
  const prefixes = loadAllowlist();

  const { failures, infos } = runChecks(en, ru, prefixes);

  console.log(`i18n check: en=${Object.keys(en).length} keys, ru=${Object.keys(ru).length} keys, ${prefixes.length} allowlisted prefix(es)`);

  if (infos.length > 0) {
    console.log(`\nINFO (${infos.length}) — untranslated keys outside the allowlist, not blocking:`);
    for (const info of infos.slice(0, 50)) console.log(`  - ${info}`);
    if (infos.length > 50) console.log(`  ... and ${infos.length - 50} more`);
  }

  // Schema-audit: config.field.* catalog vs. the live schema fixture. WARN
  // only — never gates the build (see checkFieldCatalogOrphans /
  // checkHighTrafficFieldCoverage doc comments).
  const schemaPaths = loadSchemaPaths();
  if (schemaPaths.length > 0) {
    const catalogPaths = fieldCatalogPaths(en, ru);
    const orphanWarnings = checkFieldCatalogOrphans(catalogPaths, schemaPaths);
    const coverageWarnings = checkHighTrafficFieldCoverage(
      catalogPaths,
      schemaPaths,
      HIGH_TRAFFIC_FIELD_PREFIXES,
    );
    const schemaWarnings = [...orphanWarnings, ...coverageWarnings];
    if (schemaWarnings.length > 0) {
      console.log(`\nWARN (${schemaWarnings.length}) — config.field.* schema-audit (schema-paths.json), not blocking:`);
      for (const w of schemaWarnings) console.log(`  - ${w}`);
    }
  } else {
    console.log("\n(schema-audit skipped: scripts/schema-paths.json not found)");
  }

  if (failures.length > 0) {
    console.error(`\nFAIL (${failures.length}):`);
    for (const failure of failures) console.error(`  - ${failure}`);
    process.exit(1);
  }

  console.log("\ni18n check passed.");
}

const isMain = process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url;
if (isMain) {
  main().catch((err) => {
    console.error(err);
    process.exit(1);
  });
}
