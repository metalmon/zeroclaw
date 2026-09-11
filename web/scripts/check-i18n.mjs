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

// ---------------------------------------------------------------------------
// Checks — each returns { failures: string[], infos: string[] }
// ---------------------------------------------------------------------------

function isAllowlisted(key, prefixes) {
  return prefixes.some((prefix) => key.startsWith(prefix));
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
  return {
    failures: [
      ...coverage.failures,
      ...placeholder.failures,
      ...splitKey.failures,
      ...englishLeak.failures,
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
